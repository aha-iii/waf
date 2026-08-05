//! 应用级防护模块与检测流水线的规范化持久化（PostgreSQL 专用表 + 文件目录）。

use nerve_waf_sdk::{ModuleConfig, ProtectionModules};
use nerve_waf_semantic::{
    EngineFusion, ExpertLevel, PipelineEditor, PipelineNodeLayout, SemanticConfig, SemanticMode,
    PIPELINE_STAGE_IDS,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::ConfigError;
use crate::config::AuditContext;

const DEFAULT_PIPELINE_ORDER: &[&str] = PIPELINE_STAGE_IDS;

fn semantic_mode_str(mode: &SemanticMode) -> &'static str {
    match mode {
        SemanticMode::Standard => "standard",
        SemanticMode::Strict => "strict",
        SemanticMode::Relaxed => "relaxed",
    }
}

fn parse_semantic_mode(s: &str) -> SemanticMode {
    match s {
        "strict" => SemanticMode::Strict,
        "relaxed" => SemanticMode::Relaxed,
        _ => SemanticMode::Standard,
    }
}

pub fn app_protection_file(data_dir: &Path, site_id: &str) -> PathBuf {
    data_dir.join("app_protection").join(format!("{site_id}.json"))
}

pub fn app_pipeline_file(data_dir: &Path, site_id: &str) -> PathBuf {
    data_dir.join("app_pipeline").join(format!("{site_id}.json"))
}

pub fn load_app_protection_file(data_dir: &Path, site_id: &str) -> Result<Option<ProtectionModules>, ConfigError> {
    let path = app_protection_file(data_dir, site_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw).unwrap_or_default()))
}

pub fn save_app_protection_file(
    data_dir: &Path,
    site_id: &str,
    modules: &ProtectionModules,
) -> Result<(), ConfigError> {
    let path = app_protection_file(data_dir, site_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(modules)?)?;
    Ok(())
}

pub fn load_app_pipeline_file(data_dir: &Path, site_id: &str) -> Result<Option<SemanticConfig>, ConfigError> {
    let path = app_pipeline_file(data_dir, site_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: SemanticConfig = serde_json::from_str(&raw).unwrap_or_default();
    Ok(Some(cfg.normalized()))
}

pub fn save_app_pipeline_file(
    data_dir: &Path,
    site_id: &str,
    cfg: &SemanticConfig,
) -> Result<(), ConfigError> {
    let path = app_pipeline_file(data_dir, site_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

#[cfg(feature = "postgres")]
pub mod pg {
    use super::*;
    use sqlx::{PgPool, Row};

    pub async fn migrate_legacy(pool: &PgPool) -> Result<(), ConfigError> {
        migrate_legacy_protection(pool).await?;
        migrate_legacy_pipeline(pool).await?;
        Ok(())
    }

    async fn migrate_legacy_protection(pool: &PgPool) -> Result<(), ConfigError> {
        let rows = sqlx::query("SELECT site_id, version, mode, modules, updated_by FROM waf_l7_modules")
            .fetch_all(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for row in rows {
            let site_id: String = row.get("site_id");
            let modules_val: Value = row.get("modules");
            let modules_map: HashMap<String, ModuleConfig> =
                serde_json::from_value(modules_val).unwrap_or_default();
            let protection = ProtectionModules {
                version: row.get::<i32, _>("version") as u32,
                mode: row.get("mode"),
                modules: modules_map,
            };

            if site_id == "default" {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM waf_protection WHERE id = 1)",
                )
                .fetch_one(pool)
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
                if exists {
                    continue;
                }
                crate::global_config::pg::save_global_protection(
                    pool,
                    &protection,
                    &AuditContext::system("migrate"),
                )
                .await?;
                continue;
            }

            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM waf_app_protection WHERE site_id = $1)",
            )
            .bind(&site_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
            if exists {
                continue;
            }
            save_app_protection(
                pool,
                &site_id,
                &protection,
                &AuditContext::system("migrate"),
            )
            .await?;
        }
        Ok(())
    }

    async fn migrate_legacy_pipeline(pool: &PgPool) -> Result<(), ConfigError> {
        let rows = sqlx::query("SELECT site_id, semantic FROM waf_ai_settings")
            .fetch_all(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for row in rows {
            let site_id: String = row.get("site_id");
            let semantic: Value = row.get("semantic");
            if semantic.is_null() || semantic.as_object().is_some_and(|o| o.is_empty()) {
                continue;
            }
            let cfg: SemanticConfig = serde_json::from_value(semantic).unwrap_or_default();

            if site_id == "default" {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM waf_pipeline WHERE id = 1)",
                )
                .fetch_one(pool)
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
                if exists {
                    continue;
                }
                crate::global_config::pg::save_global_pipeline(
                    pool,
                    &cfg.normalized(),
                    None,
                    &AuditContext::system("migrate"),
                )
                .await?;
                continue;
            }

            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM waf_app_pipeline WHERE site_id = $1)",
            )
            .bind(&site_id)
            .fetch_one(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
            if exists {
                continue;
            }
            save_app_pipeline(
                pool,
                &site_id,
                &cfg.normalized(),
                None,
                &AuditContext::system("migrate"),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn load_app_protection(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<ProtectionModules>, ConfigError> {
        let header = sqlx::query(
            "SELECT version, mode FROM waf_app_protection WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let Some(header) = header else {
            return Ok(None);
        };

        let module_rows = sqlx::query(
            "SELECT module_key, enabled, action, sensitivity FROM waf_app_protection_modules WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let mut modules = HashMap::new();
        for row in module_rows {
            let key: String = row.get("module_key");
            modules.insert(
                key.clone(),
                ModuleConfig {
                    enabled: row.get("enabled"),
                    action: row.get("action"),
                    sensitivity: row.get("sensitivity"),
                    disabled_rules: Vec::new(),
                    extra: HashMap::new(),
                },
            );
        }

        let rule_rows = sqlx::query(
            "SELECT module_key, rule_id FROM waf_app_protection_disabled_rules WHERE site_id = $1 ORDER BY module_key, rule_id",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        for row in rule_rows {
            let key: String = row.get("module_key");
            let rule_id: String = row.get("rule_id");
            if let Some(m) = modules.get_mut(&key) {
                m.disabled_rules.push(rule_id);
            }
        }

        Ok(Some(ProtectionModules {
            version: header.get::<i32, _>("version") as u32,
            mode: header.get("mode"),
            modules,
        }))
    }

    /// 读取系统级 Web 核心防护（`waf_protection` 表，非 waf_app_*）。
    pub async fn load_default_protection(pool: &PgPool) -> Result<ProtectionModules, ConfigError> {
        if let Some(m) = crate::global_config::pg::load_global_protection(pool).await? {
            return Ok(m);
        }
        let row = sqlx::query("SELECT version, mode, modules FROM waf_l7_modules WHERE site_id = $1")
            .bind("default")
            .fetch_optional(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        if let Some(row) = row {
            let modules_val: Value = row.get("modules");
            let modules_map: HashMap<String, ModuleConfig> =
                serde_json::from_value(modules_val).unwrap_or_default();
            let protection = ProtectionModules {
                version: row.get::<i32, _>("version") as u32,
                mode: row.get("mode"),
                modules: modules_map,
            };
            let _ = crate::global_config::pg::save_global_protection(
                pool,
                &protection,
                &AuditContext::system("migrate"),
            )
            .await;
            return Ok(protection);
        }
        Ok(ProtectionModules::default())
    }

    /// 读取生效配置：应用已落库则返回应用级完整快照，否则继承系统默认。
    pub async fn load_effective_protection(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<ProtectionModules, ConfigError> {
        if let Some(app) = load_app_protection(pool, site_id).await? {
            return Ok(app);
        }
        load_default_protection(pool).await
    }

    pub async fn delete_app_protection(pool: &PgPool, site_id: &str) -> Result<(), ConfigError> {
        if site_id == "default" {
            return Err(ConfigError::Db("cannot delete global default protection".into()));
        }
        sqlx::query("DELETE FROM waf_app_protection WHERE site_id = $1")
            .bind(site_id)
            .execute(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        let _ = sqlx::query("DELETE FROM waf_l7_modules WHERE site_id = $1")
            .bind(site_id)
            .execute(pool)
            .await;
        Ok(())
    }

    pub async fn delete_app_pipeline(pool: &PgPool, site_id: &str) -> Result<(), ConfigError> {
        if site_id == "default" {
            return Err(ConfigError::Db("cannot delete global default pipeline".into()));
        }
        sqlx::query("DELETE FROM waf_app_pipeline WHERE site_id = $1")
            .bind(site_id)
            .execute(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn save_app_protection(
        pool: &PgPool,
        site_id: &str,
        modules: &ProtectionModules,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        save_app_protection_inner(pool, site_id, modules, ctx).await
    }

    /// 从系统 `waf_pipeline*` 复制完整流水线快照（provision 专用，避免 save_app_pipeline 复杂路径失败）。
    pub async fn provision_app_pipeline_from_global(
        pool: &PgPool,
        site_id: &str,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_pipeline WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        if exists {
            return Ok(());
        }

        let op_name = ctx
            .operator_id_name
            .as_deref()
            .unwrap_or("provision");

        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_app_pipeline
               (site_id, enabled, semantic_mode, intent_enabled,
                semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                decode, scope, whitelist, performance, auto_tune, expert_level,
                created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               SELECT $1, enabled, semantic_mode, intent_enabled,
                      semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                      decode, scope, whitelist, performance, auto_tune, NULL,
                      $2, $3, $2, $3
               FROM waf_pipeline WHERE id = 1"#,
        )
        .bind(site_id)
        .bind(ctx.operator_id)
        .bind(op_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_app_pipeline_stages
               (site_id, stage_id, sort_order, enabled, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               SELECT $1, stage_id, sort_order, enabled, $2, $3, $2, $3
               FROM waf_pipeline_stages
               ON CONFLICT (site_id, stage_id) DO NOTHING"#,
        )
        .bind(site_id)
        .bind(ctx.operator_id)
        .bind(op_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_app_pipeline_editor
               (site_id, editor_mode, layout, graph, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               SELECT $1, editor_mode, layout, graph, $2, $3, $2, $3
               FROM waf_pipeline_editor WHERE id = 1
               ON CONFLICT (site_id) DO NOTHING"#,
        )
        .bind(site_id)
        .bind(ctx.operator_id)
        .bind(op_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    /// 新应用（含内置 `default` 应用）首次落库：从系统默认复制防护模块、流水线、自定义规则完整快照。
    pub async fn ensure_app_provisioned(
        pool: &PgPool,
        site_id: &str,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let protection_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_protection WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        if protection_exists {
            if let Some(app) = load_app_protection(pool, site_id).await? {
                let base = load_default_protection(pool).await?;
                if app.modules.len() < base.modules.len() {
                    let full = ProtectionModules::merge_with_overrides(&base, &app);
                    save_app_protection_inner(pool, site_id, &full, ctx).await?;
                }
            }
        } else {
            let base = load_default_protection(pool).await?;
            save_app_protection_inner(pool, site_id, &base, ctx).await?;
        }

        let pipeline_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_pipeline WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        if !pipeline_exists {
            provision_app_pipeline_from_global(pool, site_id, ctx).await?;
        }

        crate::custom_rules::pg::provision_app_custom_from_default(pool, site_id).await?;
        // 仅在尚无绑定时写入缺省「标准」；禁止每次 provision 覆盖用户已选策略
        let has_bound: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM waf_app_bound_strategy WHERE site_id = $1)",
        )
        .bind(site_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
        if !has_bound {
            set_bound_strategy(pool, site_id, DEFAULT_BOUND_STRATEGY_ID, ctx).await?;
        }
        Ok(())
    }

    async fn save_app_protection_inner(
        pool: &PgPool,
        site_id: &str,
        modules: &ProtectionModules,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_app_protection
               (site_id, version, mode, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1, $2, $3, $4, $5, $4, $5)
               ON CONFLICT (site_id) DO UPDATE SET
                 version = EXCLUDED.version,
                 mode = EXCLUDED.mode,
                 updated_at = now(),
                 updated_by_id = EXCLUDED.updated_by_id,
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(site_id)
        .bind(modules.version as i32)
        .bind(&modules.mode)
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_app_protection_modules WHERE site_id = $1")
            .bind(site_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        sqlx::query("DELETE FROM waf_app_protection_disabled_rules WHERE site_id = $1")
            .bind(site_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for (key, cfg) in &modules.modules {
            sqlx::query(
                r#"INSERT INTO waf_app_protection_modules
                   (site_id, module_key, enabled, action, sensitivity,
                    created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $6, $7)"#,
            )
            .bind(site_id)
            .bind(key)
            .bind(cfg.enabled)
            .bind(&cfg.action)
            .bind(&cfg.sensitivity)
            .bind(ctx.operator_id)
            .bind(&ctx.operator_id_name)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

            for rule_id in &cfg.disabled_rules {
                sqlx::query(
                    r#"INSERT INTO waf_app_protection_disabled_rules
                       (site_id, module_key, rule_id,
                        created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                       VALUES ($1, $2, $3, $4, $5, $4, $5)
                       ON CONFLICT (site_id, module_key, rule_id) DO UPDATE SET
                         updated_at = now(),
                         updated_by_id = EXCLUDED.updated_by_id,
                         updated_by_id_name = EXCLUDED.updated_by_id_name"#,
                )
                .bind(site_id)
                .bind(key)
                .bind(rule_id)
                .bind(ctx.operator_id)
                .bind(&ctx.operator_id_name)
                .execute(tx.as_mut())
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
            }
        }

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    /// 读取应用级流水线覆盖（不含系统默认回退）。
    pub async fn load_app_pipeline_override(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<SemanticConfig>, ConfigError> {
        let row = sqlx::query(
            r#"SELECT enabled, semantic_mode, intent_enabled,
                      semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                      decode, scope, whitelist, performance, auto_tune, expert_level
               FROM waf_app_pipeline WHERE site_id = $1"#,
        )
        .bind(site_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let stage_rows = sqlx::query(
            "SELECT stage_id, sort_order, enabled FROM waf_app_pipeline_stages WHERE site_id = $1 ORDER BY sort_order",
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let mut pipeline_order = Vec::new();
        let mut pipeline_enabled = HashMap::new();
        if stage_rows.is_empty() {
            for (i, id) in DEFAULT_PIPELINE_ORDER.iter().enumerate() {
                pipeline_order.push((*id).into());
                pipeline_enabled.insert((*id).into(), true);
                let _ = i;
            }
        } else {
            for s in &stage_rows {
                let id: String = s.get("stage_id");
                pipeline_order.push(id.clone());
                pipeline_enabled.insert(id, s.get("enabled"));
            }
        }

        let editor_row = sqlx::query(
            "SELECT editor_mode, layout, graph FROM waf_app_pipeline_editor WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let pipeline_editor = if let Some(ed) = editor_row {
            let layout_val: Value = ed.get("layout");
            let layout_map: HashMap<String, PipelineNodeLayout> =
                serde_json::from_value(layout_val).unwrap_or_default();
            PipelineEditor {
                mode: ed.get("editor_mode"),
                layout: layout_map,
                graph: ed.get("graph"),
            }
        } else {
            PipelineEditor::default()
        };

        let decode: Value = row.get("decode");
        let scope: Value = row.get("scope");
        let whitelist: Value = row.get("whitelist");
        let performance: Value = row.get("performance");
        let auto_tune: Value = row.get("auto_tune");

        let mut cfg = SemanticConfig {
            enabled: row.get("enabled"),
            mode: parse_semantic_mode(row.get::<String, _>("semantic_mode").as_str()),
            decode: serde_json::from_value(decode).unwrap_or_default(),
            scope: serde_json::from_value(scope).unwrap_or_default(),
            whitelist: serde_json::from_value(whitelist).unwrap_or_default(),
            engines: EngineFusion {
                semantic_first: row.get("semantic_first"),
                libinjection: row.get("libinjection"),
                sql_intent: row.get("sql_intent"),
                fallback_plain_libinjection: row.get("fallback_plain_libinjection"),
                pipeline_order,
                pipeline_enabled,
            },
            performance: serde_json::from_value(performance).unwrap_or_default(),
            auto_tune: serde_json::from_value(auto_tune).unwrap_or_default(),
            intent_enabled: row.get("intent_enabled"),
            pipeline_editor,
            expert_level: row
                .try_get::<Option<String>, _>("expert_level")
                .ok()
                .flatten()
                .map(|s| ExpertLevel::parse(&s)),
        };
        cfg = cfg.normalized();
        Ok(Some(cfg))
    }

    /// 生效流水线：应用 `waf_app_pipeline` 快照；未 provision 时继承系统 `waf_pipeline`。
    pub async fn load_effective_pipeline(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<SemanticConfig, ConfigError> {
        let global = crate::global_config::pg::load_global_pipeline(pool)
            .await?
            .unwrap_or_default();
        if let Some(mut ov) = load_app_pipeline_override(pool, site_id).await? {
            // 已有应用快照则不再跟全局 expert_level 联动
            if ov.expert_level.is_none() {
                ov.expert_level = Some(ExpertLevel::Medium);
            }
            return Ok(ov);
        }
        Ok(global)
    }

    /// 兼容旧名：读取生效流水线配置。
    pub async fn load_app_pipeline(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Option<SemanticConfig>, ConfigError> {
        Ok(Some(load_effective_pipeline(pool, site_id).await?))
    }

    pub async fn save_app_pipeline(
        pool: &PgPool,
        site_id: &str,
        cfg: &SemanticConfig,
        triage_llm: Option<&Value>,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let cfg = cfg.clone().normalized();
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("SET LOCAL lock_timeout = '15s'")
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1)::bigint)")
            .bind(site_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        let decode = serde_json::to_value(&cfg.decode)?;
        let scope = serde_json::to_value(&cfg.scope)?;
        let whitelist = serde_json::to_value(&cfg.whitelist)?;
        let performance = serde_json::to_value(&cfg.performance)?;
        let auto_tune = serde_json::to_value(&cfg.auto_tune)?;

        sqlx::query(
            r#"INSERT INTO waf_app_pipeline
               (site_id, enabled, semantic_mode, intent_enabled,
                semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                decode, scope, whitelist, performance, auto_tune, expert_level,
                created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$15,$16)
               ON CONFLICT (site_id) DO UPDATE SET
                 enabled = EXCLUDED.enabled,
                 semantic_mode = EXCLUDED.semantic_mode,
                 intent_enabled = EXCLUDED.intent_enabled,
                 semantic_first = EXCLUDED.semantic_first,
                 libinjection = EXCLUDED.libinjection,
                 sql_intent = EXCLUDED.sql_intent,
                 fallback_plain_libinjection = EXCLUDED.fallback_plain_libinjection,
                 decode = EXCLUDED.decode,
                 scope = EXCLUDED.scope,
                 whitelist = EXCLUDED.whitelist,
                 performance = EXCLUDED.performance,
                 auto_tune = EXCLUDED.auto_tune,
                 expert_level = EXCLUDED.expert_level,
                 updated_at = now(),
                 updated_by_id = EXCLUDED.updated_by_id,
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(site_id)
        .bind(cfg.enabled)
        .bind(semantic_mode_str(&cfg.mode))
        .bind(cfg.intent_enabled)
        .bind(cfg.engines.semantic_first)
        .bind(cfg.engines.libinjection)
        .bind(cfg.engines.sql_intent)
        .bind(cfg.engines.fallback_plain_libinjection)
        .bind(decode)
        .bind(scope)
        .bind(whitelist)
        .bind(performance)
        .bind(auto_tune)
        .bind(cfg.expert_level.map(|l| l.as_str()))
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let order = if cfg.engines.pipeline_order.is_empty() {
            DEFAULT_PIPELINE_ORDER
                .iter()
                .map(|s| (*s).into())
                .collect::<Vec<_>>()
        } else {
            cfg.engines.pipeline_order.clone()
        };

        for (idx, stage_id) in order.iter().enumerate() {
            let enabled = cfg.engines.is_pipeline_stage_enabled(stage_id);
            sqlx::query(
                r#"INSERT INTO waf_app_pipeline_stages
                   (site_id, stage_id, sort_order, enabled,
                    created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, $6, $5, $6)
                   ON CONFLICT (site_id, stage_id) DO UPDATE SET
                     sort_order = EXCLUDED.sort_order,
                     enabled = EXCLUDED.enabled,
                     updated_at = now(),
                     updated_by_id = EXCLUDED.updated_by_id,
                     updated_by_id_name = EXCLUDED.updated_by_id_name"#,
            )
            .bind(site_id)
            .bind(stage_id)
            .bind(idx as i32)
            .bind(enabled)
            .bind(ctx.operator_id)
            .bind(&ctx.operator_id_name)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }

        sqlx::query(
            "DELETE FROM waf_app_pipeline_stages WHERE site_id = $1 AND NOT (stage_id = ANY($2))",
        )
        .bind(site_id)
        .bind(&order)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let layout = serde_json::to_value(&cfg.pipeline_editor.layout)?;
        sqlx::query(
            r#"INSERT INTO waf_app_pipeline_editor
               (site_id, editor_mode, layout, graph,
                created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1, $2, $3, $4, $5, $6, $5, $6)
               ON CONFLICT (site_id) DO UPDATE SET
                 editor_mode = EXCLUDED.editor_mode,
                 layout = EXCLUDED.layout,
                 graph = EXCLUDED.graph,
                 updated_at = now(),
                 updated_by_id = EXCLUDED.updated_by_id,
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(site_id)
        .bind(&cfg.pipeline_editor.mode)
        .bind(layout)
        .bind(&cfg.pipeline_editor.graph)
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        // 兼容：waf_ai_settings 保留 JSON 快照（semantic + triage_llm 同事务写入，避免二次锁等待）
        let semantic_json = serde_json::to_value(&cfg)?;
        if let Some(triage) = triage_llm {
            sqlx::query(
                r#"INSERT INTO waf_ai_settings (site_id, semantic, triage_llm, updated_at)
                   VALUES ($1, $2, $3, now())
                   ON CONFLICT (site_id) DO UPDATE SET
                     semantic = EXCLUDED.semantic,
                     triage_llm = EXCLUDED.triage_llm,
                     updated_at = now()"#,
            )
            .bind(site_id)
            .bind(semantic_json)
            .bind(triage)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        } else {
            sqlx::query(
                r#"INSERT INTO waf_ai_settings (site_id, semantic, triage_llm, updated_at)
                   VALUES ($1, $2, '{}'::jsonb, now())
                   ON CONFLICT (site_id) DO UPDATE SET
                     semantic = EXCLUDED.semantic,
                     updated_at = now()"#,
            )
            .bind(site_id)
            .bind(semantic_json)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn get_bound_strategy(pool: &PgPool, site_id: &str) -> Result<String, ConfigError> {
        let row: Option<String> = sqlx::query_scalar(
            "SELECT strategy_id FROM waf_app_bound_strategy WHERE site_id = $1",
        )
        .bind(site_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(row
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_BOUND_STRATEGY_ID.to_string()))
    }

    pub async fn set_bound_strategy(
        pool: &PgPool,
        site_id: &str,
        strategy_id: &str,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let id = if strategy_id.trim().is_empty() {
            DEFAULT_BOUND_STRATEGY_ID
        } else {
            strategy_id.trim()
        };
        sqlx::query(
            r#"INSERT INTO waf_app_bound_strategy (site_id, strategy_id, updated_at, updated_by_id_name)
               VALUES ($1, $2, now(), $3)
               ON CONFLICT (site_id) DO UPDATE SET
                 strategy_id = EXCLUDED.strategy_id,
                 updated_at = now(),
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(site_id)
        .bind(id)
        .bind(&ctx.operator_id_name)
        .execute(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    /// 将仍绑定到已删/已禁用策略的应用回退到「标准」
    pub async fn rebind_sites_away_from_strategy(
        pool: &PgPool,
        strategy_id: &str,
        ctx: &AuditContext,
    ) -> Result<u64, ConfigError> {
        let r = sqlx::query(
            r#"UPDATE waf_app_bound_strategy
               SET strategy_id = $2, updated_at = now(), updated_by_id_name = $3
               WHERE strategy_id = $1"#,
        )
        .bind(strategy_id)
        .bind(DEFAULT_BOUND_STRATEGY_ID)
        .bind(&ctx.operator_id_name)
        .execute(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(r.rows_affected())
    }
}

pub const DEFAULT_BOUND_STRATEGY_ID: &str = "tpl-standard";

pub fn app_bound_strategy_file(data_dir: &Path, site_id: &str) -> PathBuf {
    data_dir.join("app_bound_strategy").join(format!("{site_id}.json"))
}

pub fn load_bound_strategy_file(data_dir: &Path, site_id: &str) -> Result<String, ConfigError> {
    let path = app_bound_strategy_file(data_dir, site_id);
    if !path.exists() {
        return Ok(DEFAULT_BOUND_STRATEGY_ID.to_string());
    }
    let raw = std::fs::read_to_string(&path)?;
    let v: Value = serde_json::from_str(&raw)?;
    Ok(v
        .get("strategy_id")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BOUND_STRATEGY_ID)
        .to_string())
}

pub fn save_bound_strategy_file(
    data_dir: &Path,
    site_id: &str,
    strategy_id: &str,
) -> Result<(), ConfigError> {
    let path = app_bound_strategy_file(data_dir, site_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::json!({ "strategy_id": strategy_id });
    std::fs::write(&path, serde_json::to_string_pretty(&body)?)?;
    Ok(())
}

/// 文件模式：凡绑定到 strategy_id 的应用回退到标准
pub fn rebind_sites_away_from_strategy_file(
    data_dir: &Path,
    strategy_id: &str,
) -> Result<u64, ConfigError> {
    let dir = data_dir.join("app_bound_strategy");
    if !dir.exists() {
        return Ok(0);
    }
    let mut n = 0u64;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let site = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if site.is_empty() {
            continue;
        }
        let cur = load_bound_strategy_file(data_dir, &site)?;
        if cur == strategy_id {
            save_bound_strategy_file(data_dir, &site, DEFAULT_BOUND_STRATEGY_ID)?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_file_roundtrip() {
        let dir = std::env::temp_dir().join("nerve_app_cfg_test");
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = SemanticConfig::default();
        save_app_pipeline_file(&dir, "demo", &cfg).unwrap();
        let loaded = load_app_pipeline_file(&dir, "demo").unwrap().unwrap();
        assert_eq!(loaded.enabled, cfg.enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bound_strategy_file_defaults() {
        let dir = std::env::temp_dir().join("nerve_bound_strat_test");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            load_bound_strategy_file(&dir, "demo").unwrap(),
            DEFAULT_BOUND_STRATEGY_ID
        );
        save_bound_strategy_file(&dir, "demo", "tpl-strict").unwrap();
        assert_eq!(load_bound_strategy_file(&dir, "demo").unwrap(), "tpl-strict");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
