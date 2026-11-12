//! Per-app CRS version binding and history snapshots.

use nerve_waf_sdk::{ModuleConfig, ProtectionModules};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::ConfigError;

/// CRS-related Web core protection module keys.
pub const CRS_MODULE_KEYS: &[&str] = &[
    "sqli",
    "xss",
    "rce",
    "path_traversal",
    "ssrf",
    "webshell",
    "file_upload",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppCrsBinding {
    pub site_id: String,
    pub crs_version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_version_label: Option<String>,
    pub rule_count: u32,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppCrsHistoryEntry {
    pub id: String,
    pub site_id: String,
    pub crs_version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_version_label: Option<String>,
    pub rule_count: u32,
    pub modules_snapshot: HashMap<String, ModuleConfig>,
    pub is_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCrsStatus {
    pub binding: Option<AppCrsBinding>,
    pub system_latest: Option<SystemCrsVersionRef>,
    pub behind_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCrsVersionRef {
    pub crs_version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_version_label: Option<String>,
    pub rule_count: u32,
}

pub fn crs_modules_snapshot(modules: &ProtectionModules) -> HashMap<String, ModuleConfig> {
    CRS_MODULE_KEYS
        .iter()
        .filter_map(|k| {
            modules
                .modules
                .get(*k)
                .map(|c| ((*k).to_string(), c.clone()))
        })
        .collect()
}

pub fn prune_disabled_rules(
    modules: &mut HashMap<String, ModuleConfig>,
    valid_rule_ids: &HashSet<String>,
) {
    for cfg in modules.values_mut() {
        cfg.disabled_rules.retain(|id| valid_rule_ids.contains(id));
    }
}

pub fn app_crs_dir(data_dir: &Path, site_id: &str) -> std::path::PathBuf {
    data_dir.join("app_crs").join(site_id)
}

pub fn binding_file(data_dir: &Path, site_id: &str) -> std::path::PathBuf {
    app_crs_dir(data_dir, site_id).join("binding.json")
}

pub fn history_file(data_dir: &Path, site_id: &str) -> std::path::PathBuf {
    app_crs_dir(data_dir, site_id).join("history.json")
}

pub fn load_binding_file(
    data_dir: &Path,
    site_id: &str,
) -> Result<Option<AppCrsBinding>, ConfigError> {
    let path = binding_file(data_dir, site_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn save_binding_file(data_dir: &Path, binding: &AppCrsBinding) -> Result<(), ConfigError> {
    let path = binding_file(data_dir, &binding.site_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(binding)?)?;
    Ok(())
}

pub fn load_history_file(
    data_dir: &Path,
    site_id: &str,
) -> Result<Vec<AppCrsHistoryEntry>, ConfigError> {
    let path = history_file(data_dir, site_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save_history_file(
    data_dir: &Path,
    site_id: &str,
    entries: &[AppCrsHistoryEntry],
) -> Result<(), ConfigError> {
    let path = history_file(data_dir, site_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

#[cfg(feature = "postgres")]
pub mod pg {
    use super::*;
    use sqlx::{PgPool, Row};

    pub async fn load_binding(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<AppCrsBinding>, ConfigError> {
        let row = sqlx::query(
            "SELECT site_id, crs_version_id, crs_version_label, rule_count, applied_at::text \
             FROM waf_app_crs_binding WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(row.map(|r| AppCrsBinding {
            site_id: r.get("site_id"),
            crs_version_id: r.get("crs_version_id"),
            crs_version_label: r.get("crs_version_label"),
            rule_count: r.get::<i32, _>("rule_count") as u32,
            applied_at: r.get("applied_at"),
        }))
    }

    pub async fn load_history(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Vec<AppCrsHistoryEntry>, ConfigError> {
        let rows = sqlx::query(
            "SELECT waf_app_crs_history_id::text AS id, site_id, crs_version_id, crs_version_label, \
                    rule_count, modules_snapshot, is_active, note, applied_at::text \
             FROM waf_app_crs_history WHERE site_id = $1 ORDER BY applied_at DESC",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let snap: serde_json::Value = r.get("modules_snapshot");
                AppCrsHistoryEntry {
                    id: r.get("id"),
                    site_id: r.get("site_id"),
                    crs_version_id: r.get("crs_version_id"),
                    crs_version_label: r.get("crs_version_label"),
                    rule_count: r.get::<i32, _>("rule_count") as u32,
                    modules_snapshot: serde_json::from_value(snap).unwrap_or_default(),
                    is_active: r.get("is_active"),
                    note: r.get("note"),
                    applied_at: r.get("applied_at"),
                }
            })
            .collect())
    }

    pub async fn load_history_entry(
        pool: &PgPool,
        site_id: &str,
        history_id: &str,
    ) -> Result<Option<AppCrsHistoryEntry>, ConfigError> {
        let row = sqlx::query(
            "SELECT waf_app_crs_history_id::text AS id, site_id, crs_version_id, crs_version_label, \
                    rule_count, modules_snapshot, is_active, note, applied_at::text \
             FROM waf_app_crs_history WHERE site_id = $1 AND waf_app_crs_history_id::text = $2",
        )
        .bind(site_id)
        .bind(history_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(row.map(|r| {
            let snap: serde_json::Value = r.get("modules_snapshot");
            AppCrsHistoryEntry {
                id: r.get("id"),
                site_id: r.get("site_id"),
                crs_version_id: r.get("crs_version_id"),
                crs_version_label: r.get("crs_version_label"),
                rule_count: r.get::<i32, _>("rule_count") as u32,
                modules_snapshot: serde_json::from_value(snap).unwrap_or_default(),
                is_active: r.get("is_active"),
                note: r.get("note"),
                applied_at: r.get("applied_at"),
            }
        }))
    }

    pub async fn apply_version(
        pool: &PgPool,
        site_id: &str,
        crs_version_id: &str,
        crs_version_label: Option<&str>,
        rule_count: u32,
        modules_snapshot: HashMap<String, ModuleConfig>,
        operator: &str,
    ) -> Result<AppCrsBinding, ConfigError> {
        let archive_modules =
            if let Some(prot) = crate::app_config::pg::load_app_protection(pool, site_id).await? {
                crs_modules_snapshot(&prot)
            } else {
                modules_snapshot.clone()
            };

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        if let Some(old) = load_binding(pool, site_id).await? {
            sqlx::query(
                "UPDATE waf_app_crs_history SET is_active = false WHERE site_id = $1 AND is_active = true",
            )
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

            let old_modules = archive_modules;

            sqlx::query(
                r#"INSERT INTO waf_app_crs_history
                   (site_id, crs_version_id, crs_version_label, rule_count, modules_snapshot, is_active, note, applied_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, false, $6, $7)"#,
            )
            .bind(site_id)
            .bind(&old.crs_version_id)
            .bind(&old.crs_version_label)
            .bind(old.rule_count as i32)
            .bind(serde_json::to_value(&old_modules).map_err(|e| ConfigError::Db(e.to_string()))?)
            .bind(format!("切换前自动归档 · {}", old.crs_version_id))
            .bind(operator)
            .execute(&mut *tx)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        } else {
            sqlx::query(
                "UPDATE waf_app_crs_history SET is_active = false WHERE site_id = $1 AND is_active = true",
            )
            .bind(site_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }

        sqlx::query(
            r#"INSERT INTO waf_app_crs_history
               (site_id, crs_version_id, crs_version_label, rule_count, modules_snapshot, is_active, applied_by_id_name)
               VALUES ($1, $2, $3, $4, $5, true, $6)"#,
        )
        .bind(site_id)
        .bind(crs_version_id)
        .bind(crs_version_label)
        .bind(rule_count as i32)
        .bind(serde_json::to_value(&modules_snapshot).map_err(|e| ConfigError::Db(e.to_string()))?)
        .bind(operator)
        .execute(&mut *tx)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_app_crs_binding (site_id, crs_version_id, crs_version_label, rule_count, applied_by_id_name)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (site_id) DO UPDATE SET
                 crs_version_id = EXCLUDED.crs_version_id,
                 crs_version_label = EXCLUDED.crs_version_label,
                 rule_count = EXCLUDED.rule_count,
                 applied_at = now(),
                 applied_by_id_name = EXCLUDED.applied_by_id_name"#,
        )
        .bind(site_id)
        .bind(crs_version_id)
        .bind(crs_version_label)
        .bind(rule_count as i32)
        .bind(operator)
        .execute(&mut *tx)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        load_binding(pool, site_id)
            .await?
            .ok_or_else(|| ConfigError::Db("binding missing after apply".into()))
    }
}
