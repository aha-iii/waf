use nerve_waf_l7::{CustomRule, CustomRuleSet, CustomRulesFile};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// Sidebar 'Custom rules' system-library scope (`waf_custom_*` tables; separate from app `waf_app_custom_*`).
pub const GLOBAL_CUSTOM_SITE_ID: &str = "global";

#[derive(Debug, Error)]
pub enum CustomRulesError {
    #[error("database: {0}")]
    Db(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

pub struct CustomRulesStore {
    data_dir: PathBuf,
    global_file_path: PathBuf,
    #[cfg(feature = "postgres")]
    pool: Option<sqlx::PgPool>,
}

impl CustomRulesStore {
    pub async fn open(database_url: Option<&str>, data_dir: &Path) -> Result<Self, CustomRulesError> {
        let global_file_path = data_dir.join("custom_rules.json");
        #[cfg(feature = "postgres")]
        let pool = if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
            Some(
                crate::migrations::connect_pool(url)
                    .await
                    .map_err(|e| CustomRulesError::Db(e.to_string()))?,
            )
        } else {
            None
        };
        #[cfg(not(feature = "postgres"))]
        let _ = database_url;

        let store = Self {
            data_dir: data_dir.to_path_buf(),
            global_file_path,
            #[cfg(feature = "postgres")]
            pool,
        };
        store.ensure_global_seeded().await?;
        Ok(store)
    }

    fn app_file_path(&self, site_id: &str) -> PathBuf {
        self.data_dir
            .join("app_custom_rules")
            .join(format!("{site_id}.json"))
    }

    async fn ensure_global_seeded(&self) -> Result<(), CustomRulesError> {
        let data = self.load_global().await?;
        if !data.sets.is_empty() {
            return Ok(());
        }
        let seeded = default_custom_rules();
        self.save_global(&seeded).await
    }

    /// System-level custom rule library (sidebar 'Custom rules').
    pub async fn load_global(&self) -> Result<CustomRulesFile, CustomRulesError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            return pg::load_global_custom_all(pool, GLOBAL_CUSTOM_SITE_ID).await;
        }
        self.load_from_file(&self.global_file_path)
    }

    pub async fn save_global(&self, data: &CustomRulesFile) -> Result<(), CustomRulesError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            pg::save_global_custom_all(pool, GLOBAL_CUSTOM_SITE_ID, data).await?;
            return Ok(());
        }
        self.save_to_file(&self.global_file_path, data)
    }

    /// Per-app copy; if not provisioned, return system template (read-only inherit, not persisted).
    pub async fn load_all_for(&self, site_id: &str) -> Result<CustomRulesFile, CustomRulesError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            if let Some(data) = pg::load_app_custom_all(pool, site_id).await? {
                return Ok(data);
            }
            return pg::load_global_custom_all(pool, GLOBAL_CUSTOM_SITE_ID).await;
        }
        let app_path = self.app_file_path(site_id);
        if app_path.exists() {
            return self.load_from_file(&app_path);
        }
        self.load_global().await
    }

    /// Full write of per-app custom-rule copy.
    pub async fn save_all_for(&self, site_id: &str, data: &CustomRulesFile) -> Result<(), CustomRulesError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            return pg::save_app_custom_all(pool, site_id, data).await;
        }
        self.save_to_file(&self.app_file_path(site_id), data)
    }

    pub async fn load_active_rules_for(&self, site_id: &str) -> Result<Vec<CustomRule>, CustomRulesError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            if let Some(rules) = pg::load_app_custom_active_rules(pool, site_id).await? {
                return Ok(rules);
            }
            return pg::load_global_active_rules(pool).await;
        }
        let data = self.load_all_for(site_id).await?;
        Self::active_rules_from_file(&data)
    }

    fn load_from_file(&self, path: &Path) -> Result<CustomRulesFile, CustomRulesError> {
        if !path.exists() {
            return Ok(CustomRulesFile::default());
        }
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn save_to_file(&self, path: &Path, data: &CustomRulesFile) -> Result<(), CustomRulesError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(data)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    fn active_rules_from_file(data: &CustomRulesFile) -> Result<Vec<CustomRule>, CustomRulesError> {
        let enabled_sets: std::collections::HashSet<_> = data
            .sets
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        Ok(data
            .rules
            .iter()
            .filter(|r| {
                r.enabled
                    && !r.rule_set_id.is_empty()
                    && enabled_sets.contains(r.rule_set_id.as_str())
            })
            .cloned()
            .collect())
    }
}

#[cfg(feature = "postgres")]
pub mod pg {
    use super::*;
    use sqlx::{PgPool, Row};

    async fn resolve_global_custom_scope(pool: &PgPool) -> Result<String, CustomRulesError> {
        let global_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_custom_rule_sets WHERE site_id = $1",
        )
        .bind(GLOBAL_CUSTOM_SITE_ID)
        .fetch_one(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        if global_count > 0 {
            return Ok(GLOBAL_CUSTOM_SITE_ID.into());
        }
        let legacy_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_custom_rule_sets WHERE site_id = 'default'",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        if legacy_count > 0 {
            return Ok("default".into());
        }
        Ok(GLOBAL_CUSTOM_SITE_ID.into())
    }

    pub async fn provision_app_custom_from_default(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<(), CustomRulesError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_custom_rule_sets WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        if exists {
            return Ok(());
        }

        let scope = resolve_global_custom_scope(pool).await?;

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        sqlx::query(
            "INSERT INTO waf_app_custom_rule_sets (site_id, set_id, name, description, enabled, created_by_id_name, updated_by_id_name) \
             SELECT $1, id, name, description, enabled, 'provision', 'provision' \
             FROM waf_custom_rule_sets WHERE site_id = $2 \
             ON CONFLICT (site_id, set_id) DO NOTHING",
        )
        .bind(site_id)
        .bind(&scope)
        .execute(&mut *tx)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        sqlx::query(
            "INSERT INTO waf_app_custom_rules \
             (site_id, rule_id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern, action, severity, category, rule_type, plugin_name, created_by_id_name, updated_by_id_name) \
             SELECT $1, r.id, r.rule_set_id, r.name, r.phase, r.priority, r.enabled, \
                    COALESCE(r.match_field, ''), COALESCE(r.match_op, ''), COALESCE(r.match_pattern, ''), \
                    r.action, r.severity, r.category, COALESCE(r.rule_type, 'signature'), COALESCE(r.plugin_name, ''), \
                    'provision', 'provision' \
             FROM waf_custom_rules r \
             INNER JOIN waf_custom_rule_sets s ON s.id = r.rule_set_id AND s.site_id = $2 \
             ON CONFLICT (site_id, rule_id) DO NOTHING",
        )
        .bind(site_id)
        .bind(&scope)
        .execute(&mut *tx)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn save_global_custom_all(
        pool: &PgPool,
        scope: &str,
        data: &CustomRulesFile,
    ) -> Result<(), CustomRulesError> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        sqlx::query(
            "DELETE FROM waf_custom_rules WHERE rule_set_id IN (SELECT id FROM waf_custom_rule_sets WHERE site_id = $1) \
             OR rule_set_id IS NULL OR rule_set_id = ''",
        )
        .bind(scope)
        .execute(&mut *tx)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        sqlx::query("DELETE FROM waf_custom_rule_sets WHERE site_id = $1")
            .bind(scope)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        for s in &data.sets {
            sqlx::query(
                "INSERT INTO waf_custom_rule_sets (id, site_id, name, description, enabled) VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(&s.id)
            .bind(scope)
            .bind(&s.name)
            .bind(&s.description)
            .bind(s.enabled)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        }

        for r in &data.rules {
            let set_id: Option<&str> = if r.rule_set_id.is_empty() {
                None
            } else {
                Some(r.rule_set_id.as_str())
            };
            sqlx::query(
                "INSERT INTO waf_custom_rules (id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern, action, severity, category, rule_type, plugin_name) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
            )
            .bind(&r.id)
            .bind(set_id)
            .bind(&r.name)
            .bind(&r.phase)
            .bind(r.priority)
            .bind(r.enabled)
            .bind(&r.match_field)
            .bind(&r.match_op)
            .bind(&r.match_pattern)
            .bind(&r.action)
            .bind(&r.severity)
            .bind(&r.category)
            .bind(&r.rule_type)
            .bind(&r.plugin_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn save_app_custom_all(
        pool: &PgPool,
        site_id: &str,
        data: &CustomRulesFile,
    ) -> Result<(), CustomRulesError> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_app_custom_rules WHERE site_id = $1")
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        sqlx::query("DELETE FROM waf_app_custom_rule_sets WHERE site_id = $1")
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        for s in &data.sets {
            sqlx::query(
                "INSERT INTO waf_app_custom_rule_sets (site_id, set_id, name, description, enabled, created_by_id_name, updated_by_id_name) \
                 VALUES ($1,$2,$3,$4,$5,'console','console')",
            )
            .bind(site_id)
            .bind(&s.id)
            .bind(&s.name)
            .bind(&s.description)
            .bind(s.enabled)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        }

        for r in &data.rules {
            sqlx::query(
                "INSERT INTO waf_app_custom_rules \
                 (site_id, rule_id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern, action, severity, category, rule_type, plugin_name, created_by_id_name, updated_by_id_name) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'console','console')",
            )
            .bind(site_id)
            .bind(&r.id)
            .bind(&r.rule_set_id)
            .bind(&r.name)
            .bind(&r.phase)
            .bind(r.priority)
            .bind(r.enabled)
            .bind(&r.match_field)
            .bind(&r.match_op)
            .bind(&r.match_pattern)
            .bind(&r.action)
            .bind(&r.severity)
            .bind(&r.category)
            .bind(&r.rule_type)
            .bind(&r.plugin_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn load_app_custom_all(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<CustomRulesFile>, CustomRulesError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_custom_rule_sets WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        if !exists {
            return Ok(None);
        }

        let set_rows = sqlx::query(
            "SELECT set_id, name, COALESCE(description, '') AS description, enabled \
             FROM waf_app_custom_rule_sets WHERE site_id = $1 ORDER BY name",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let sets = set_rows
            .into_iter()
            .map(|r| CustomRuleSet {
                id: r.get("set_id"),
                name: r.get("name"),
                description: r.try_get::<String, _>("description").unwrap_or_default(),
                enabled: r.get("enabled"),
            })
            .collect();

        let rule_rows = sqlx::query(
            "SELECT rule_id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern, action, severity, category, \
             COALESCE(rule_type, 'signature') AS rule_type, COALESCE(plugin_name, '') AS plugin_name \
             FROM waf_app_custom_rules WHERE site_id = $1 ORDER BY priority, name",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let rules = rule_rows
            .into_iter()
            .map(|r| CustomRule {
                id: r.get("rule_id"),
                rule_set_id: r.get("rule_set_id"),
                name: r.get("name"),
                phase: r.get("phase"),
                priority: r.get("priority"),
                enabled: r.get("enabled"),
                match_field: r.get("match_field"),
                match_op: r.get("match_op"),
                match_pattern: r.get("match_pattern"),
                action: r.get("action"),
                severity: r.get("severity"),
                category: r.get("category"),
                rule_type: r.try_get("rule_type").unwrap_or_else(|_| "signature".into()),
                plugin_name: r.try_get("plugin_name").unwrap_or_default(),
            })
            .collect();

        Ok(Some(CustomRulesFile { sets, rules }))
    }

    pub async fn load_global_custom_all(
        pool: &PgPool,
        scope: &str,
    ) -> Result<CustomRulesFile, CustomRulesError> {
        let set_rows = sqlx::query(
            "SELECT id, name, COALESCE(description, '') AS description, enabled FROM waf_custom_rule_sets WHERE site_id = $1 ORDER BY name",
        )
        .bind(scope)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let sets = set_rows
            .into_iter()
            .map(|r| CustomRuleSet {
                id: r.get("id"),
                name: r.get("name"),
                description: r.try_get::<String, _>("description").unwrap_or_default(),
                enabled: r.get("enabled"),
            })
            .collect();

        let rule_rows = sqlx::query(
            "SELECT r.id, COALESCE(r.rule_set_id, '') AS rule_set_id, r.name, r.phase, r.priority, r.enabled, \
             COALESCE(r.match_field, '') AS match_field, COALESCE(r.match_op, '') AS match_op, \
             COALESCE(r.match_pattern, '') AS match_pattern, r.action, r.severity, r.category, \
             COALESCE(r.rule_type, 'signature') AS rule_type, COALESCE(r.plugin_name, '') AS plugin_name \
             FROM waf_custom_rules r \
             LEFT JOIN waf_custom_rule_sets s ON s.id = r.rule_set_id \
             WHERE s.site_id = $1 OR r.rule_set_id IS NULL OR r.rule_set_id = '' \
             ORDER BY r.priority, r.name",
        )
        .bind(scope)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let rules = rule_rows
            .into_iter()
            .map(|r| CustomRule {
                id: r.get("id"),
                rule_set_id: r.get("rule_set_id"),
                name: r.get("name"),
                phase: r.get("phase"),
                priority: r.get("priority"),
                enabled: r.get("enabled"),
                match_field: r.get("match_field"),
                match_op: r.get("match_op"),
                match_pattern: r.get("match_pattern"),
                action: r.get("action"),
                severity: r.get("severity"),
                category: r.get("category"),
                rule_type: r.try_get("rule_type").unwrap_or_else(|_| "signature".into()),
                plugin_name: r.try_get("plugin_name").unwrap_or_default(),
            })
            .collect();

        Ok(CustomRulesFile { sets, rules })
    }

    pub async fn load_app_custom_active_rules(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<Vec<CustomRule>>, CustomRulesError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_custom_rule_sets WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;
        if !exists {
            return Ok(None);
        }

        let set_rows = sqlx::query(
            "SELECT set_id, enabled FROM waf_app_custom_rule_sets WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let enabled_sets: std::collections::HashSet<String> = set_rows
            .iter()
            .filter(|r| r.get::<bool, _>("enabled"))
            .map(|r| r.get::<String, _>("set_id"))
            .collect();

        let rule_rows = sqlx::query(
            "SELECT rule_id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern, action, severity, category, \
             COALESCE(rule_type, 'signature') AS rule_type, COALESCE(plugin_name, '') AS plugin_name \
             FROM waf_app_custom_rules WHERE site_id = $1 ORDER BY priority, name",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| CustomRulesError::Db(e.to_string()))?;

        let rules = rule_rows
            .into_iter()
            .filter(|r| {
                r.get::<bool, _>("enabled")
                    && enabled_sets.contains(&r.get::<String, _>("rule_set_id"))
            })
            .map(|r| CustomRule {
                id: r.get("rule_id"),
                rule_set_id: r.get("rule_set_id"),
                name: r.get("name"),
                phase: r.get("phase"),
                priority: r.get("priority"),
                enabled: r.get("enabled"),
                match_field: r.get("match_field"),
                match_op: r.get("match_op"),
                match_pattern: r.get("match_pattern"),
                action: r.get("action"),
                severity: r.get("severity"),
                category: r.get("category"),
                rule_type: r.try_get("rule_type").unwrap_or_else(|_| "signature".into()),
                plugin_name: r.try_get("plugin_name").unwrap_or_default(),
            })
            .collect();
        Ok(Some(rules))
    }

    pub async fn load_global_active_rules(pool: &PgPool) -> Result<Vec<CustomRule>, CustomRulesError> {
        let data = load_global_custom_all(pool, GLOBAL_CUSTOM_SITE_ID).await?;
        let enabled_sets: std::collections::HashSet<_> = data
            .sets
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        Ok(data
            .rules
            .into_iter()
            .filter(|r| {
                r.enabled
                    && (!r.rule_set_id.is_empty() && enabled_sets.contains(r.rule_set_id.as_str())
                        || r.rule_set_id.is_empty())
            })
            .collect())
    }
}

pub fn new_rule_id() -> String {
    format!("custom-{}", Uuid::new_v4())
}

pub fn new_set_id() -> String {
    format!("set-{}", Uuid::new_v4())
}

pub fn default_custom_rules() -> CustomRulesFile {
    CustomRulesFile {
        sets: vec![CustomRuleSet {
            id: "default".into(),
            name: "默认自定义规则".into(),
            description: "Console 可编辑的签名规则".into(),
            enabled: true,
        }],
        rules: vec![
            CustomRule {
                id: "custom-xff-len".into(),
                rule_set_id: "default".into(),
                name: "XFF 过长".into(),
                phase: "request".into(),
                priority: 10,
                enabled: true,
                match_field: "headers".into(),
                match_op: "regex".into(),
                match_pattern: "(?i)x-forwarded-for:\\s*.{100,}".into(),
                action: "block".into(),
                severity: "medium".into(),
                category: "custom".into(),
                rule_type: "signature".into(),
                plugin_name: String::new(),
            },
            CustomRule {
                id: "custom-uri-sqlmap".into(),
                rule_set_id: "default".into(),
                name: "URI 含 sqlmap".into(),
                phase: "request".into(),
                priority: 20,
                enabled: true,
                match_field: "uri".into(),
                match_op: "contains".into(),
                match_pattern: "sqlmap".into(),
                action: "block".into(),
                severity: "high".into(),
                category: "custom".into(),
                rule_type: "signature".into(),
                plugin_name: String::new(),
            },
        ],
    }
}
