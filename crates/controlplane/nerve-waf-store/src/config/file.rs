use super::{ConfigError, RuntimeDenyEntry};
use chrono::{DateTime, Utc};
use nerve_waf_sdk::{L7Upstream, ProtectionModules, WafConfig, WafRoute};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct FileConfigStore {
    data_dir: PathBuf,
    config_path: PathBuf,
    modules_path: PathBuf,
    upstreams_path: PathBuf,
    runtime_denies_path: PathBuf,
    example_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct UpstreamsFile {
    #[serde(default)]
    upstreams: Vec<L7Upstream>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RuntimeDenyFile {
    denies: Vec<RuntimeDenyRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeDenyRecord {
    ip: String,
    source: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl FileConfigStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            config_path: data_dir.join("routes.json"),
            modules_path: data_dir.join("modules.json"),
            upstreams_path: data_dir.join("upstreams.json"),
            runtime_denies_path: data_dir.join("runtime_denies.json"),
            example_path: Self::find_example_path(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn modules_path(&self) -> &Path {
        &self.modules_path
    }

    pub fn upstreams_path(&self) -> &Path {
        &self.upstreams_path
    }

    fn find_example_path() -> PathBuf {
        for path in [
            PathBuf::from("examples/routes.json"),
            PathBuf::from("../examples/routes.json"),
            PathBuf::from("../../examples/routes.json"),
        ] {
            if path.exists() {
                return path;
            }
        }
        PathBuf::from("examples/routes.json")
    }

    pub fn load_config(&self) -> Result<WafConfig, ConfigError> {
        if self.config_path.exists() {
            let raw = fs::read_to_string(&self.config_path)?;
            Ok(serde_json::from_str(&raw).unwrap_or_default())
        } else if self.example_path.exists() {
            let raw = fs::read_to_string(&self.example_path)?;
            Ok(serde_json::from_str(&raw).unwrap_or_default())
        } else {
            Ok(WafConfig {
                version: 1,
                routes: vec![],
            })
        }
    }

    pub fn save_config(&self, config: &WafConfig) -> Result<(), ConfigError> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, raw)?;
        Ok(())
    }

    pub fn load_modules(&self) -> Result<ProtectionModules, ConfigError> {
        let site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        self.load_modules_for(&site_id)
    }

    pub fn load_modules_for(&self, site_id: &str) -> Result<ProtectionModules, ConfigError> {
        if let Some(app) = crate::app_config::load_app_protection_file(&self.data_dir, site_id)? {
            return Ok(app);
        }
        let legacy = self.modules_path_for(site_id);
        if legacy.exists() {
            let raw = fs::read_to_string(&legacy)?;
            return Ok(serde_json::from_str(&raw).unwrap_or_default());
        }
        self.load_global_modules()
    }

    pub fn load_global_modules(&self) -> Result<ProtectionModules, ConfigError> {
        if self.modules_path.exists() {
            let raw = fs::read_to_string(&self.modules_path)?;
            return Ok(serde_json::from_str(&raw).unwrap_or_default());
        }
        Ok(ProtectionModules::default())
    }

    fn global_pipeline_path(&self) -> PathBuf {
        self.data_dir.join("global_pipeline.json")
    }

    pub fn load_global_pipeline(&self) -> Result<nerve_waf_semantic::SemanticConfig, ConfigError> {
        let path = self.global_pipeline_path();
        if !path.exists() {
            return Ok(nerve_waf_semantic::SemanticConfig::default());
        }
        let raw = fs::read_to_string(&path)?;
        let cfg: nerve_waf_semantic::SemanticConfig =
            serde_json::from_str(&raw).unwrap_or_default();
        Ok(cfg.normalized())
    }

    pub fn save_global_pipeline(
        &self,
        cfg: &nerve_waf_semantic::SemanticConfig,
    ) -> Result<(), ConfigError> {
        let path = self.global_pipeline_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(cfg)?)?;
        Ok(())
    }

    pub fn save_global_modules(&self, modules: &ProtectionModules) -> Result<(), ConfigError> {
        if let Some(parent) = self.modules_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(modules)?;
        fs::write(&self.modules_path, raw)?;
        Ok(())
    }

    pub fn save_modules(&self, modules: &ProtectionModules) -> Result<(), ConfigError> {
        let site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        self.save_modules_for(&site_id, modules)
    }

    pub fn save_modules_for(
        &self,
        site_id: &str,
        modules: &ProtectionModules,
    ) -> Result<(), ConfigError> {
        self.ensure_app_provisioned(site_id)?;
        crate::app_config::save_app_protection_file(&self.data_dir, site_id, modules)
    }

    pub fn ensure_app_provisioned(&self, site_id: &str) -> Result<(), ConfigError> {
        if crate::app_config::load_app_protection_file(&self.data_dir, site_id)?.is_none() {
            let base = self.load_global_modules()?;
            crate::app_config::save_app_protection_file(&self.data_dir, site_id, &base)?;
        }
        if crate::app_config::load_app_pipeline_file(&self.data_dir, site_id)?.is_none() {
            let base = self.load_global_pipeline()?;
            crate::app_config::save_app_pipeline_file(&self.data_dir, site_id, &base)?;
        }
        // Default bind to 'Standard'.
        if !crate::app_config::app_bound_strategy_file(&self.data_dir, site_id).exists() {
            crate::app_config::save_bound_strategy_file(
                &self.data_dir,
                site_id,
                crate::app_config::DEFAULT_BOUND_STRATEGY_ID,
            )?;
        }
        Ok(())
    }

    fn modules_path_for(&self, site_id: &str) -> PathBuf {
        self.data_dir
            .join("modules")
            .join(format!("{site_id}.json"))
    }

    pub fn load_upstreams(&self) -> Result<Vec<L7Upstream>, ConfigError> {
        if self.upstreams_path.exists() {
            let raw = fs::read_to_string(&self.upstreams_path)?;
            let file: UpstreamsFile = serde_json::from_str(&raw).unwrap_or_default();
            Ok(file.upstreams)
        } else {
            Ok(default_upstreams())
        }
    }

    pub fn save_upstreams(&self, upstreams: &[L7Upstream]) -> Result<(), ConfigError> {
        if let Some(parent) = self.upstreams_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = UpstreamsFile {
            upstreams: upstreams.to_vec(),
        };
        let raw = serde_json::to_string_pretty(&file)?;
        fs::write(&self.upstreams_path, raw)?;
        Ok(())
    }

    pub fn load_gateway_dataplane(&self) -> Result<(Vec<WafRoute>, Vec<L7Upstream>), ConfigError> {
        let config = self.load_config()?;
        let routes: Vec<WafRoute> = config
            .routes
            .into_iter()
            .filter(|r| {
                r.enabled
                    && (r.layer == "l7"
                        || r.match_.l7.as_ref().is_some_and(|l7| {
                            l7.host.is_some()
                                || l7.path_prefix.is_some()
                                || l7.methods.as_ref().is_some_and(|m| !m.is_empty())
                                || l7.uri_regex.is_some()
                        }))
            })
            .collect();
        let upstreams: Vec<L7Upstream> = self
            .load_upstreams()?
            .into_iter()
            .filter(|u| u.enabled)
            .collect();
        Ok((routes, upstreams))
    }

    fn load_runtime_file(&self) -> Result<RuntimeDenyFile, ConfigError> {
        if !self.runtime_denies_path.exists() {
            return Ok(RuntimeDenyFile::default());
        }
        let raw = fs::read_to_string(&self.runtime_denies_path)?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    fn save_runtime_file(&self, file: &RuntimeDenyFile) -> Result<(), ConfigError> {
        if let Some(parent) = self.runtime_denies_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(file)?;
        fs::write(&self.runtime_denies_path, raw)?;
        Ok(())
    }

    fn is_active(record: &RuntimeDenyRecord, now: DateTime<Utc>) -> bool {
        record.expires_at.map(|exp| exp > now).unwrap_or(true)
    }

    pub fn active_runtime_deny_ips(&self) -> Result<Vec<String>, ConfigError> {
        let now = Utc::now();
        Ok(self
            .load_runtime_file()?
            .denies
            .iter()
            .filter(|d| Self::is_active(d, now))
            .map(|d| d.ip.clone())
            .collect())
    }

    pub fn list_runtime_denies(&self) -> Result<Vec<RuntimeDenyEntry>, ConfigError> {
        let now = Utc::now();
        Ok(self
            .load_runtime_file()?
            .denies
            .iter()
            .filter(|d| Self::is_active(d, now))
            .map(|d| RuntimeDenyEntry {
                ip: d.ip.clone(),
                source: d.source.clone(),
                reason: d.reason.clone(),
                expires_at: d.expires_at,
                created_at: d.created_at,
            })
            .collect())
    }

    pub fn add_runtime_deny(
        &self,
        ip: &str,
        source: &str,
        reason: Option<&str>,
        _operator: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), ConfigError> {
        let mut file = self.load_runtime_file()?;
        file.denies.retain(|d| d.ip != ip);
        file.denies.push(RuntimeDenyRecord {
            ip: ip.into(),
            source: source.into(),
            reason: reason.map(str::to_string),
            expires_at,
            created_at: Utc::now(),
        });
        self.save_runtime_file(&file)
    }

    pub fn remove_runtime_deny(&self, ip: &str) -> Result<(), ConfigError> {
        let mut file = self.load_runtime_file()?;
        let before = file.denies.len();
        file.denies.retain(|d| d.ip != ip);
        if file.denies.len() != before {
            self.save_runtime_file(&file)?;
        }
        Ok(())
    }

    pub fn load_app_crs_binding(
        &self,
        site_id: &str,
    ) -> Result<Option<crate::AppCrsBinding>, ConfigError> {
        crate::app_crs::load_binding_file(&self.data_dir, site_id)
    }

    pub fn load_app_crs_history(
        &self,
        site_id: &str,
    ) -> Result<Vec<crate::AppCrsHistoryEntry>, ConfigError> {
        crate::app_crs::load_history_file(&self.data_dir, site_id)
    }

    pub fn load_app_crs_history_entry(
        &self,
        site_id: &str,
        history_id: &str,
    ) -> Result<Option<crate::AppCrsHistoryEntry>, ConfigError> {
        Ok(crate::app_crs::load_history_file(&self.data_dir, site_id)?
            .into_iter()
            .find(|e| e.id == history_id))
    }

    pub fn apply_app_crs_version(
        &self,
        site_id: &str,
        crs_version_id: &str,
        crs_version_label: Option<&str>,
        rule_count: u32,
        modules_snapshot: std::collections::HashMap<String, nerve_waf_sdk::ModuleConfig>,
        operator: &str,
    ) -> Result<crate::AppCrsBinding, ConfigError> {
        let mut history = crate::app_crs::load_history_file(&self.data_dir, site_id)?;
        let archive_modules = crate::app_config::load_app_protection_file(&self.data_dir, site_id)?
            .map(|m| crate::crs_modules_snapshot(&m))
            .unwrap_or_else(|| modules_snapshot.clone());
        if let Some(old) = crate::app_crs::load_binding_file(&self.data_dir, site_id)? {
            for h in history.iter_mut() {
                h.is_active = false;
            }
            let archived_ver = old.crs_version_id.clone();
            history.push(crate::AppCrsHistoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                site_id: site_id.to_string(),
                crs_version_id: old.crs_version_id,
                crs_version_label: old.crs_version_label,
                rule_count: old.rule_count,
                modules_snapshot: archive_modules,
                is_active: false,
                note: Some(format!("切换前自动归档 · {}", archived_ver)),
                applied_at: chrono::Utc::now().to_rfc3339(),
            });
        } else {
            for h in history.iter_mut() {
                h.is_active = false;
            }
        }
        let applied_at = chrono::Utc::now().to_rfc3339();
        history.insert(
            0,
            crate::AppCrsHistoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                site_id: site_id.to_string(),
                crs_version_id: crs_version_id.to_string(),
                crs_version_label: crs_version_label.map(str::to_string),
                rule_count,
                modules_snapshot,
                is_active: true,
                note: None,
                applied_at: applied_at.clone(),
            },
        );
        let binding = crate::AppCrsBinding {
            site_id: site_id.to_string(),
            crs_version_id: crs_version_id.to_string(),
            crs_version_label: crs_version_label.map(str::to_string),
            rule_count,
            applied_at,
        };
        crate::app_crs::save_history_file(&self.data_dir, site_id, &history)?;
        crate::app_crs::save_binding_file(&self.data_dir, &binding)?;
        let _ = operator;
        Ok(binding)
    }
}

fn default_upstreams() -> Vec<L7Upstream> {
    vec![L7Upstream {
        id: "default".into(),
        name: "默认后端".into(),
        enabled: true,
        scheme: "http".into(),
        upstream_type: "roundrobin".into(),
        nodes: vec![nerve_waf_sdk::L7UpstreamNode {
            host: "127.0.0.1".into(),
            port: 8080,
            weight: 1,
        }],
        description: Some("Gateway 默认 upstream".into()),
        host_header: None,
        sni: None,
        transport: None,
        origin_kind: Some("direct".into()),
        checks: None,
        health_check: None,
    }]
}
