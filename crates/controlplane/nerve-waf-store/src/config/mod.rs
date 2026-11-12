mod file;

#[cfg(feature = "postgres")]
mod postgres;

use nerve_waf_sdk::{L7Upstream, ProtectionModules, WafConfig};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub use file::FileConfigStore;

#[cfg(feature = "postgres")]
pub use postgres::PostgresConfigStore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeDenyEntry {
    pub ip: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("database: {0}")]
    Db(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<crate::custom_rules::CustomRulesError> for ConfigError {
    fn from(value: crate::custom_rules::CustomRulesError) -> Self {
        ConfigError::Db(value.to_string())
    }
}

impl From<ConfigError> for crate::StoreError {
    fn from(value: ConfigError) -> Self {
        match value {
            ConfigError::Db(msg) => crate::StoreError::Db(msg),
            ConfigError::Io(e) => crate::StoreError::Io(e),
            ConfigError::Json(e) => crate::StoreError::Json(e),
        }
    }
}

/// Console/API operator (writes created_by_* / updated_by_*).
#[derive(Debug, Clone, Default)]
pub struct AuditContext {
    pub operator_id: Option<Uuid>,
    pub operator_id_name: Option<String>,
}

impl AuditContext {
    pub fn console() -> Self {
        Self {
            operator_id: None,
            operator_id_name: Some("控制台".into()),
        }
    }

    pub fn system(name: &str) -> Self {
        Self {
            operator_id: None,
            operator_id_name: Some(name.into()),
        }
    }

    pub fn from_legacy_operator(operator: &str) -> Self {
        if let Ok(id) = Uuid::parse_str(operator.trim()) {
            Self {
                operator_id: Some(id),
                operator_id_name: None,
            }
        } else {
            Self {
                operator_id: None,
                operator_id_name: Some(operator.into()),
            }
        }
    }
}

#[derive(Clone)]
pub enum ConfigBackend {
    File(FileConfigStore),
    #[cfg(feature = "postgres")]
    Postgres(PostgresConfigStore),
}

impl ConfigBackend {
    pub fn file(data_dir: &Path) -> Self {
        Self::File(FileConfigStore::new(data_dir))
    }

    #[cfg(feature = "postgres")]
    pub fn postgres(store: PostgresConfigStore) -> Self {
        Self::Postgres(store)
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::File(_) => "file",
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgresql",
        }
    }

    pub fn config_source(&self) -> String {
        match self {
            Self::File(s) => s.config_path().display().to_string(),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgresql:waf_l3_policies+waf_l7_routes".into(),
        }
    }

    pub fn modules_source(&self) -> String {
        match self {
            Self::File(s) => s.modules_path().display().to_string(),
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgresql:waf_l7_modules".into(),
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        match self {
            Self::File(s) => s.data_dir().to_path_buf(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.data_dir().to_path_buf(),
        }
    }

    pub async fn load_config(&self) -> Result<WafConfig, ConfigError> {
        match self {
            Self::File(s) => s.load_config(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_config().await,
        }
    }

    pub async fn save_config(&self, config: &WafConfig, operator: &str) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.save_config(config),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_config(config, operator).await,
        }
    }

    pub async fn load_modules(&self) -> Result<ProtectionModules, ConfigError> {
        let site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        self.load_modules_for(&site_id).await
    }

    pub async fn load_modules_for(&self, site_id: &str) -> Result<ProtectionModules, ConfigError> {
        match self {
            Self::File(s) => s.load_modules_for(site_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_modules_for(site_id).await,
        }
    }

    pub async fn save_modules(
        &self,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        self.save_modules_for(&site_id, modules, operator).await
    }

    pub async fn save_modules_for(
        &self,
        site_id: &str,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.save_modules_for(site_id, modules),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_modules_for(site_id, modules, operator).await,
        }
    }

    /// System Web core protection template (`waf_protection`) for new-app provision and unconfigured inherit.
    pub async fn load_global_modules(&self) -> Result<ProtectionModules, ConfigError> {
        match self {
            Self::File(s) => s.load_global_modules(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_global_modules().await,
        }
    }

    pub async fn save_global_modules(
        &self,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.save_global_modules(modules),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_global_modules(modules, operator).await,
        }
    }

    /// System detect-pipeline template (`waf_pipeline`).
    pub async fn load_global_pipeline(
        &self,
    ) -> Result<nerve_waf_semantic::SemanticConfig, ConfigError> {
        match self {
            Self::File(s) => s.load_global_pipeline(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_global_pipeline().await,
        }
    }

    pub async fn save_global_pipeline(
        &self,
        cfg: &nerve_waf_semantic::SemanticConfig,
        operator: &str,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.save_global_pipeline(cfg),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_global_pipeline(cfg, operator).await,
        }
    }

    /// Before first save of a new app, copy full config snapshot from system defaults into app tables.
    pub async fn ensure_app_provisioned(&self, site_id: &str) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.ensure_app_provisioned(site_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.ensure_app_provisioned(site_id).await,
        }
    }

    pub async fn active_runtime_deny_ips(&self) -> Result<Vec<String>, ConfigError> {
        match self {
            Self::File(s) => s.active_runtime_deny_ips(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.active_runtime_deny_ips().await,
        }
    }

    pub async fn add_runtime_deny(
        &self,
        ip: &str,
        source: &str,
        reason: Option<&str>,
        operator: Option<&str>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.add_runtime_deny(ip, source, reason, operator, expires_at),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                s.add_runtime_deny(ip, source, reason, operator, expires_at)
                    .await
            }
        }
    }

    pub async fn remove_runtime_deny(&self, ip: &str) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.remove_runtime_deny(ip),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.remove_runtime_deny(ip).await,
        }
    }

    pub async fn list_runtime_denies(&self) -> Result<Vec<RuntimeDenyEntry>, ConfigError> {
        match self {
            Self::File(s) => s.list_runtime_denies(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.list_runtime_denies().await,
        }
    }

    #[cfg(feature = "postgres")]
    pub async fn import_from_files_if_empty(&self) -> Result<bool, ConfigError> {
        if let Self::Postgres(s) = self {
            s.import_from_files_if_empty().await
        } else {
            Ok(false)
        }
    }

    pub async fn load_upstreams(&self) -> Result<Vec<L7Upstream>, ConfigError> {
        match self {
            Self::File(s) => s.load_upstreams(),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_upstreams().await,
        }
    }

    pub async fn save_upstreams(
        &self,
        upstreams: &[L7Upstream],
        operator: &str,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => s.save_upstreams(upstreams),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.save_upstreams(upstreams, operator).await,
        }
    }

    pub async fn load_app_crs_binding(
        &self,
        site_id: &str,
    ) -> Result<Option<crate::AppCrsBinding>, ConfigError> {
        match self {
            Self::File(s) => s.load_app_crs_binding(site_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_app_crs_binding(site_id).await,
        }
    }

    pub async fn load_app_crs_history(
        &self,
        site_id: &str,
    ) -> Result<Vec<crate::AppCrsHistoryEntry>, ConfigError> {
        match self {
            Self::File(s) => s.load_app_crs_history(site_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_app_crs_history(site_id).await,
        }
    }

    pub async fn load_app_crs_history_entry(
        &self,
        site_id: &str,
        history_id: &str,
    ) -> Result<Option<crate::AppCrsHistoryEntry>, ConfigError> {
        match self {
            Self::File(s) => s.load_app_crs_history_entry(site_id, history_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.load_app_crs_history_entry(site_id, history_id).await,
        }
    }

    pub async fn apply_app_crs_version(
        &self,
        site_id: &str,
        crs_version_id: &str,
        crs_version_label: Option<&str>,
        rule_count: u32,
        modules_snapshot: std::collections::HashMap<String, nerve_waf_sdk::ModuleConfig>,
        operator: &str,
    ) -> Result<crate::AppCrsBinding, ConfigError> {
        match self {
            Self::File(s) => s.apply_app_crs_version(
                site_id,
                crs_version_id,
                crs_version_label,
                rule_count,
                modules_snapshot,
                operator,
            ),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                s.apply_app_crs_version(
                    site_id,
                    crs_version_id,
                    crs_version_label,
                    rule_count,
                    modules_snapshot,
                    operator,
                )
                .await
            }
        }
    }

    pub async fn list_protection_strategies(
        &self,
        for_site: Option<&str>,
    ) -> Result<Vec<crate::ProtectionStrategyTemplate>, ConfigError> {
        match self {
            Self::File(s) => {
                let modules = self.load_global_modules().await?;
                let pipeline = self.load_global_pipeline().await?;
                crate::protection_strategy::list_file(s.data_dir(), &modules, &pipeline, for_site)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                // Fast path: if strategies already exist in DB, list them without seed OnceCell (avoid init hang killing the list).
                match crate::protection_strategy::pg::list_ready(s.pool(), for_site).await {
                    Ok(list) if !list.is_empty() => {
                        crate::protection_strategy::pg::mark_ready();
                        return Ok(list);
                    }
                    Ok(list) if for_site.is_some() => return Ok(list),
                    Ok(_) | Err(_) => {}
                }
                self.ensure_protection_strategy_store().await?;
                crate::protection_strategy::pg::list_ready(s.pool(), for_site).await
            }
        }
    }

    pub async fn get_protection_strategy(
        &self,
        id: &str,
    ) -> Result<Option<crate::ProtectionStrategyTemplate>, ConfigError> {
        match self {
            Self::File(s) => {
                let modules = self.load_global_modules().await?;
                let pipeline = self.load_global_pipeline().await?;
                crate::protection_strategy::get_file(s.data_dir(), id, &modules, &pipeline)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                self.ensure_protection_strategy_store().await?;
                crate::protection_strategy::pg::get_ready(s.pool(), id).await
            }
        }
    }

    pub async fn upsert_protection_strategy(
        &self,
        template: crate::ProtectionStrategyTemplate,
    ) -> Result<crate::ProtectionStrategyTemplate, ConfigError> {
        match self {
            Self::File(s) => {
                let modules = self.load_global_modules().await?;
                let pipeline = self.load_global_pipeline().await?;
                crate::protection_strategy::upsert_file(s.data_dir(), template, &modules, &pipeline)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                self.ensure_protection_strategy_store().await?;
                crate::protection_strategy::pg::upsert_ready(s.pool(), template).await
            }
        }
    }

    /// Load global modules/pipeline as seed only on first strategy-library access; hot path never re-reads them.
    #[cfg(feature = "postgres")]
    async fn ensure_protection_strategy_store(&self) -> Result<(), ConfigError> {
        let Self::Postgres(s) = self else {
            return Ok(());
        };
        if crate::protection_strategy::pg::is_ready() {
            return Ok(());
        }
        let modules = self.load_global_modules().await?;
        let pipeline = self.load_global_pipeline().await?;
        crate::protection_strategy::pg::ensure_seeds(s.pool(), &modules, &pipeline).await
    }

    /// Create strategy: deep-copy four-section snapshot from current global runtime.
    /// `site_id = None` → global strategy library; `Some(site)` → that app's private strategy table.
    /// `source_id`: if set, deep-copy four sections from that strategy; else from current global runtime.
    /// `global_modules`: snapshot already copied from global with CC/Bot params frozen; else load global modules inside store.
    pub async fn create_protection_strategy_from_global(
        &self,
        name: &str,
        description: &str,
        site_id: Option<&str>,
        source_id: Option<&str>,
        global_modules: Option<nerve_waf_sdk::ProtectionModules>,
    ) -> Result<crate::ProtectionStrategyTemplate, ConfigError> {
        let pipeline = self.load_global_pipeline().await?;
        let id = format!("tpl-{}", uuid::Uuid::new_v4());
        let mut template = if let Some(src) = source_id.map(|s| s.trim()).filter(|s| !s.is_empty())
        {
            let Some(mut src_tpl) = self.get_protection_strategy(src).await? else {
                return Err(ConfigError::Db(format!("源策略不存在: {src}")));
            };
            src_tpl.id = id;
            src_tpl.name = name.trim().to_string();
            src_tpl.description = description.trim().to_string();
            src_tpl.builtin = false;
            src_tpl.sort_order = 100;
            src_tpl.site_id = None;
            src_tpl.sync_summary_fields();
            src_tpl
        } else {
            let modules = match global_modules {
                Some(m) => m,
                None => self.load_global_modules().await?,
            };
            crate::ProtectionStrategyTemplate::from_global_runtime(
                id,
                name.trim().to_string(),
                description.trim().to_string(),
                &pipeline,
                &modules,
            )
        };
        if let Some(site) = site_id.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            template.site_id = Some(site.to_string());
            template.builtin = false;
        }
        let saved = self.upsert_protection_strategy(template).await?;
        if site_id.map(|s| !s.trim().is_empty()).unwrap_or(false) && saved.is_global() {
            return Err(ConfigError::Db(
                "创建本应用策略失败：结果未写入应用私有表，请确认已重启 API 并完成迁移".into(),
            ));
        }
        Ok(saved)
    }

    /// Apply strategy onto global runtime (pipeline + modules).
    pub async fn apply_protection_strategy_global(
        &self,
        id: &str,
        operator: &str,
    ) -> Result<crate::ProtectionStrategyTemplate, ConfigError> {
        let tpl = self
            .get_protection_strategy(id)
            .await?
            .ok_or_else(|| ConfigError::Db(format!("策略不存在: {id}")))?;
        let pipeline = tpl.to_pipeline_patch();
        self.save_global_pipeline(&pipeline, operator).await?;
        self.save_global_modules(&tpl.core, operator).await?;
        Ok(tpl)
    }

    /// Protection strategy id currently bound to the app (default tpl-standard).
    pub async fn get_app_bound_strategy(&self, site_id: &str) -> Result<String, ConfigError> {
        match self {
            Self::File(s) => crate::app_config::load_bound_strategy_file(s.data_dir(), site_id),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => crate::app_config::pg::get_bound_strategy(s.pool(), site_id).await,
        }
    }

    /// Record app↔strategy binding (binding table only; do not change module snapshots).
    pub async fn set_app_bound_strategy(
        &self,
        site_id: &str,
        strategy_id: &str,
        operator: &str,
    ) -> Result<(), ConfigError> {
        match self {
            Self::File(s) => {
                crate::app_config::save_bound_strategy_file(s.data_dir(), site_id, strategy_id)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                let ctx = AuditContext::from_legacy_operator(operator);
                crate::app_config::pg::set_bound_strategy(s.pool(), site_id, strategy_id, &ctx)
                    .await
            }
        }
    }

    pub async fn rebind_sites_away_from_strategy(
        &self,
        strategy_id: &str,
        operator: &str,
    ) -> Result<u64, ConfigError> {
        match self {
            Self::File(s) => {
                crate::app_config::rebind_sites_away_from_strategy_file(s.data_dir(), strategy_id)
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                let ctx = AuditContext::from_legacy_operator(operator);
                crate::app_config::pg::rebind_sites_away_from_strategy(s.pool(), strategy_id, &ctx)
                    .await
            }
        }
    }

    pub async fn delete_protection_strategy(&self, id: &str) -> Result<bool, ConfigError> {
        let ok = match self {
            Self::File(s) => crate::protection_strategy::delete_file(s.data_dir(), id)?,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => {
                self.ensure_protection_strategy_store().await?;
                crate::protection_strategy::pg::delete_ready(s.pool(), id).await?
            }
        };
        if ok {
            let _ = self.rebind_sites_away_from_strategy(id, "console").await;
        }
        Ok(ok)
    }
}

pub async fn open_config_backend(
    database_url: Option<&str>,
    data_dir: &Path,
) -> Result<ConfigBackend, ConfigError> {
    #[cfg(feature = "postgres")]
    if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
        let store = PostgresConfigStore::connect(url, data_dir).await?;
        let backend = ConfigBackend::Postgres(store);
        let imported = backend.import_from_files_if_empty().await?;
        if imported {
            tracing::info!("config store: imported routes/modules from files into PostgreSQL");
        }
        tracing::info!("config store: PostgreSQL");
        return Ok(backend);
    }

    tracing::info!(
        "config store: file ({})",
        data_dir.join("routes.json").display()
    );
    Ok(ConfigBackend::file(data_dir))
}
