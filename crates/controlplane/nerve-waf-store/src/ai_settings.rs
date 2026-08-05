use nerve_waf_semantic::SemanticConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriageLlmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_auto")]
    pub auto_explain: bool,
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_auto() -> bool {
    true
}

fn default_api_url() -> String {
    "https://api.openai.com/v1/chat/completions".into()
}

fn default_model() -> String {
    "gpt-4o-mini".into()
}

impl Default for TriageLlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_explain: true,
            api_url: default_api_url(),
            api_key: String::new(),
            model: default_model(),
        }
    }
}

impl TriageLlmConfig {
    pub fn from_env_overlay(mut self) -> Self {
        if let Ok(v) = std::env::var("NERVE_LLM_ENABLED") {
            self.enabled = v == "1" || v.eq_ignore_ascii_case("true");
        }
        if let Ok(v) = std::env::var("NERVE_LLM_AUTO_EXPLAIN") {
            self.auto_explain = v != "0" && !v.eq_ignore_ascii_case("false");
        }
        if let Ok(v) = std::env::var("NERVE_LLM_API_URL") {
            if !v.is_empty() {
                self.api_url = v;
            }
        }
        if let Ok(v) = std::env::var("NERVE_LLM_API_KEY") {
            if !v.is_empty() {
                self.api_key = v;
            }
        }
        if let Ok(v) = std::env::var("NERVE_LLM_MODEL") {
            if !v.is_empty() {
                self.model = v;
            }
        }
        self
    }

    pub fn llm_ready(&self) -> bool {
        self.enabled && !self.api_key.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiSettings {
    #[serde(default)]
    pub semantic: SemanticConfig,
    #[serde(default)]
    pub triage_llm: TriageLlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AiSettingsFileFormat {
    Legacy(AiSettings),
    Multi { sites: HashMap<String, AiSettings> },
}

#[derive(Debug, Error)]
pub enum AiSettingsError {
    #[error("database: {0}")]
    Db(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config: {0}")]
    Config(#[from] crate::config::ConfigError),
}

pub struct AiSettingsStore {
    file_path: PathBuf,
    #[cfg(feature = "postgres")]
    pool: Option<sqlx::PgPool>,
    default_site_id: String,
    cache: RwLock<HashMap<String, AiSettings>>,
    /// 串行化 pipeline 写入，避免并发 PUT /api/semantic/config 在 PG 上互等锁。
    save_lock: tokio::sync::Mutex<()>,
}

impl AiSettingsStore {
    pub async fn open(database_url: Option<&str>, data_dir: &Path) -> Result<Self, AiSettingsError> {
        let file_path = data_dir.join("ai_settings.json");
        #[cfg(feature = "postgres")]
        let pool = if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
            Some(
                crate::migrations::connect_pool(url)
                    .await
                    .map_err(|e| AiSettingsError::Db(e.to_string()))?,
            )
        } else {
            None
        };
        #[cfg(not(feature = "postgres"))]
        let _ = database_url;

        let default_site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        let settings = Self {
            file_path,
            #[cfg(feature = "postgres")]
            pool,
            default_site_id,
            cache: RwLock::new(HashMap::new()),
            save_lock: tokio::sync::Mutex::new(()),
        };
        let loaded = settings.load_all_sites().await?;
        *settings.cache.write().map_err(|e| AiSettingsError::Db(e.to_string()))? = loaded;
        Ok(settings)
    }

    pub fn backend_name(&self) -> &'static str {
        #[cfg(feature = "postgres")]
        if self.pool.is_some() {
            return "postgresql";
        }
        "file"
    }

    pub fn default_site_id(&self) -> &str {
        &self.default_site_id
    }

    pub fn semantic_config(&self) -> SemanticConfig {
        self.semantic_config_for(&self.default_site_id)
    }

    pub fn semantic_config_for(&self, site_id: &str) -> SemanticConfig {
        self.cache
            .read()
            .ok()
            .and_then(|c| {
                c.get(site_id)
                    .or_else(|| c.get("default"))
                    .map(|s| s.semantic.clone())
            })
            .unwrap_or_default()
    }

    pub fn triage_llm_config(&self) -> TriageLlmConfig {
        self.cache
            .read()
            .ok()
            .and_then(|c| {
                c.get(&self.default_site_id)
                    .or_else(|| c.get("default"))
                    .map(|s| s.triage_llm.clone().from_env_overlay())
            })
            .unwrap_or_else(|| TriageLlmConfig::default().from_env_overlay())
    }

    pub async fn set_semantic(&self, cfg: SemanticConfig) -> Result<(), AiSettingsError> {
        self.set_semantic_for(&self.default_site_id, cfg).await
    }

    pub async fn set_semantic_for(
        &self,
        site_id: &str,
        cfg: SemanticConfig,
    ) -> Result<(), AiSettingsError> {
        let _guard = self.save_lock.lock().await;
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            crate::app_config::pg::ensure_app_provisioned(
                pool,
                site_id,
                &crate::config::AuditContext::console(),
            )
            .await
            .map_err(|e| AiSettingsError::Db(e.to_string()))?;
        }
        let snapshot = {
            let mut cache = self.cache.write().map_err(|e| AiSettingsError::Db(e.to_string()))?;
            let entry = cache
                .entry(site_id.to_string())
                .or_insert_with(AiSettings::default);
            entry.semantic = cfg;
            entry.clone()
        };
        self.save_site(site_id, &snapshot).await
    }

    /// 从持久化层刷新单应用语义配置缓存（provision 或外部写入后调用）。
    pub async fn refresh_site_from_store(&self, site_id: &str) -> Result<(), AiSettingsError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            let cfg = crate::app_config::pg::load_effective_pipeline(pool, site_id)
                .await
                .map_err(|e| AiSettingsError::Db(e.to_string()))?;
            let mut cache = self.cache.write().map_err(|e| AiSettingsError::Db(e.to_string()))?;
            cache.entry(site_id.to_string()).or_default().semantic = cfg;
            return Ok(());
        }
        let data_dir = self
            .file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("data"));
        if let Ok(Some(cfg)) = crate::app_config::load_app_pipeline_file(&data_dir, site_id) {
            let mut cache = self.cache.write().map_err(|e| AiSettingsError::Db(e.to_string()))?;
            cache.entry(site_id.to_string()).or_default().semantic = cfg;
        }
        Ok(())
    }

    pub async fn set_triage_llm(&self, cfg: TriageLlmConfig) -> Result<(), AiSettingsError> {
        let site_id = self.default_site_id.clone();
        let snapshot = {
            let mut cache = self.cache.write().map_err(|e| AiSettingsError::Db(e.to_string()))?;
            let entry = cache
                .entry(site_id.clone())
                .or_insert_with(AiSettings::default);
            entry.triage_llm = cfg;
            entry.clone()
        };
        self.save_site(&site_id, &snapshot).await
    }

    pub async fn load_all_sites(&self) -> Result<HashMap<String, AiSettings>, AiSettingsError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            return self.load_all_from_pg(pool).await;
        }
        self.load_all_from_file()
    }

    async fn save_site(&self, site_id: &str, data: &AiSettings) -> Result<(), AiSettingsError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            let triage = serde_json::to_value(&data.triage_llm)
                .map_err(|e| AiSettingsError::Db(e.to_string()))?;
            crate::app_config::pg::save_app_pipeline(
                pool,
                site_id,
                &data.semantic,
                Some(&triage),
                &crate::config::AuditContext::console(),
            )
            .await
            .map_err(|e| AiSettingsError::Db(e.to_string()))?;
            return Ok(());
        }
        let data_dir = self
            .file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("data"));
        crate::app_config::save_app_pipeline_file(&data_dir, site_id, &data.semantic)
            .map_err(|e| AiSettingsError::Db(e.to_string()))?;
        self.save_all_to_file()
    }

    fn load_all_from_file(&self) -> Result<HashMap<String, AiSettings>, AiSettingsError> {
        let data_dir = self
            .file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("data"));
        let mut map = if self.file_path.exists() {
            let raw = std::fs::read_to_string(&self.file_path)?;
            parse_settings_file(&raw)?
        } else {
            let mut m = HashMap::new();
            m.insert("default".into(), AiSettings::default());
            m
        };
        for (site_id, entry) in map.iter_mut() {
            if let Ok(Some(pipeline)) =
                crate::app_config::load_app_pipeline_file(&data_dir, site_id)
            {
                entry.semantic = pipeline;
            }
        }
        Ok(map)
    }

    fn save_all_to_file(&self) -> Result<(), AiSettingsError> {
        let cache = self.cache.read().map_err(|e| AiSettingsError::Db(e.to_string()))?;
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let wrapped = AiSettingsFileFormat::Multi {
            sites: cache.clone(),
        };
        std::fs::write(
            &self.file_path,
            serde_json::to_string_pretty(&wrapped)?,
        )?;
        Ok(())
    }

    #[cfg(feature = "postgres")]
    async fn load_all_from_pg(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<HashMap<String, AiSettings>, AiSettingsError> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT site_id, semantic, triage_llm FROM waf_ai_settings")
            .fetch_all(pool)
            .await
            .map_err(|e| AiSettingsError::Db(e.to_string()))?;

        if rows.is_empty() {
            let defaults = AiSettings::default();
            self.save_site("default", &defaults).await?;
            let mut m = HashMap::new();
            m.insert("default".into(), defaults);
            return Ok(m);
        }

        let mut map = HashMap::new();
        for row in rows {
            let site_id: String = row.get("site_id");
            let triage_llm: serde_json::Value = row.get("triage_llm");
            let semantic = if let Some(cfg) =
                crate::app_config::pg::load_app_pipeline(pool, &site_id).await?
            {
                cfg
            } else {
                let semantic_val: serde_json::Value = row.get("semantic");
                serde_json::from_value(semantic_val).unwrap_or_default()
            };
            map.insert(
                site_id,
                AiSettings {
                    semantic,
                    triage_llm: serde_json::from_value(triage_llm).unwrap_or_default(),
                },
            );
        }
        Ok(map)
    }
}

fn parse_settings_file(raw: &str) -> Result<HashMap<String, AiSettings>, AiSettingsError> {
    let parsed: AiSettingsFileFormat = serde_json::from_str(raw)?;
    Ok(match parsed {
        AiSettingsFileFormat::Legacy(settings) => {
            let mut m = HashMap::new();
            m.insert("default".into(), settings);
            m
        }
        AiSettingsFileFormat::Multi { sites } => sites,
    })
}
