//! System-level WAF config (`waf_protection` / `waf_pipeline`), separate from `waf_app_*` overrides.

use nerve_waf_sdk::{ModuleConfig, ProtectionModules};
use nerve_waf_semantic::{
    EngineFusion, ExpertLevel, PipelineEditor, PipelineNodeLayout, SemanticConfig, SemanticMode,
};
use serde_json::Value;
use std::collections::HashMap;

use crate::config::{AuditContext, ConfigError};

const GLOBAL_ROW_ID: i16 = 1;
const DEFAULT_PIPELINE_ORDER: &[&str] = nerve_waf_semantic::PIPELINE_STAGE_IDS;

pub fn semantic_mode_str(mode: &SemanticMode) -> &'static str {
    match mode {
        SemanticMode::Standard => "standard",
        SemanticMode::Strict => "strict",
        SemanticMode::Relaxed => "relaxed",
    }
}

pub fn parse_semantic_mode(s: &str) -> SemanticMode {
    match s {
        "strict" => SemanticMode::Strict,
        "relaxed" => SemanticMode::Relaxed,
        _ => SemanticMode::Standard,
    }
}

#[cfg(feature = "postgres")]
pub mod pg {
    use super::*;
    use sqlx::{PgPool, Row};

    pub async fn load_global_protection(
        pool: &PgPool,
    ) -> Result<Option<ProtectionModules>, ConfigError> {
        let header = sqlx::query("SELECT version, mode FROM waf_protection WHERE id = $1")
            .bind(GLOBAL_ROW_ID)
            .fetch_optional(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        let Some(header) = header else {
            return Ok(None);
        };

        let module_rows = sqlx::query(
            "SELECT module_key, enabled, action, sensitivity FROM waf_protection_modules ORDER BY module_key",
        )
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
            "SELECT module_key, rule_id FROM waf_protection_disabled_rules ORDER BY module_key, rule_id",
        )
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

    pub async fn save_global_protection(
        pool: &PgPool,
        modules: &ProtectionModules,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_protection
               (id, version, mode, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1, $2, $3, $4, $5, $4, $5)
               ON CONFLICT (id) DO UPDATE SET
                 version = EXCLUDED.version,
                 mode = EXCLUDED.mode,
                 updated_at = now(),
                 updated_by_id = EXCLUDED.updated_by_id,
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(GLOBAL_ROW_ID)
        .bind(modules.version as i32)
        .bind(&modules.mode)
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_protection_modules")
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        sqlx::query("DELETE FROM waf_protection_disabled_rules")
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for (key, cfg) in &modules.modules {
            sqlx::query(
                r#"INSERT INTO waf_protection_modules
                   (module_key, enabled, action, sensitivity,
                    created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, $6, $5, $6)"#,
            )
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
                    r#"INSERT INTO waf_protection_disabled_rules
                       (module_key, rule_id, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                       VALUES ($1, $2, $3, $4, $3, $4)
                       ON CONFLICT (module_key, rule_id) DO NOTHING"#,
                )
                .bind(key)
                .bind(rule_id)
                .bind(ctx.operator_id)
                .bind(&ctx.operator_id_name)
                .execute(tx.as_mut())
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
            }
        }

        let modules_json = serde_json::to_value(&modules.modules)?;
        let legacy_by = ctx
            .operator_id_name
            .clone()
            .or_else(|| ctx.operator_id.map(|u| u.to_string()))
            .unwrap_or_else(|| "console".into());
        sqlx::query(
            r#"INSERT INTO waf_l7_modules (site_id, version, mode, modules, updated_at, updated_by)
               VALUES ('default',$1,$2,$3,now(),$4)
               ON CONFLICT (site_id) DO UPDATE SET
                 version = EXCLUDED.version,
                 mode = EXCLUDED.mode,
                 modules = EXCLUDED.modules,
                 updated_at = now(),
                 updated_by = EXCLUDED.updated_by"#,
        )
        .bind(modules.version as i32)
        .bind(&modules.mode)
        .bind(modules_json)
        .bind(legacy_by)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn load_global_pipeline(
        pool: &PgPool,
    ) -> Result<Option<SemanticConfig>, ConfigError> {
        let row = sqlx::query(
            r#"SELECT enabled, semantic_mode, intent_enabled,
                      semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                      decode, scope, whitelist, performance, auto_tune,
                      COALESCE(expert_level, 'medium') AS expert_level
               FROM waf_pipeline WHERE id = $1"#,
        )
        .bind(GLOBAL_ROW_ID)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let stage_rows = sqlx::query(
            "SELECT stage_id, sort_order, enabled FROM waf_pipeline_stages ORDER BY sort_order",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let (pipeline_order, pipeline_enabled) = stages_from_rows(&stage_rows);

        let editor_row =
            sqlx::query("SELECT editor_mode, layout, graph FROM waf_pipeline_editor WHERE id = $1")
                .bind(GLOBAL_ROW_ID)
                .fetch_optional(pool)
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(Some(build_semantic_config(
            row,
            &pipeline_order,
            &pipeline_enabled,
            editor_row,
        )?))
    }

    pub async fn save_global_pipeline(
        pool: &PgPool,
        cfg: &SemanticConfig,
        triage_llm: Option<&Value>,
        ctx: &AuditContext,
    ) -> Result<(), ConfigError> {
        let cfg = cfg.clone().normalized();
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        let decode = serde_json::to_value(&cfg.decode)?;
        let scope = serde_json::to_value(&cfg.scope)?;
        let whitelist = serde_json::to_value(&cfg.whitelist)?;
        let performance = serde_json::to_value(&cfg.performance)?;
        let auto_tune = serde_json::to_value(&cfg.auto_tune)?;

        sqlx::query(
            r#"INSERT INTO waf_pipeline
               (id, enabled, semantic_mode, intent_enabled,
                semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
                decode, scope, whitelist, performance, auto_tune, expert_level,
                created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$15,$16)
               ON CONFLICT (id) DO UPDATE SET
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
        .bind(GLOBAL_ROW_ID)
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
        .bind(cfg.expert_level.unwrap_or(ExpertLevel::Medium).as_str())
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let order = pipeline_order(&cfg);
        for (idx, stage_id) in order.iter().enumerate() {
            let enabled = cfg.engines.is_pipeline_stage_enabled(stage_id);
            sqlx::query(
                r#"INSERT INTO waf_pipeline_stages
                   (stage_id, sort_order, enabled, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, $4, $5)
                   ON CONFLICT (stage_id) DO UPDATE SET
                     sort_order = EXCLUDED.sort_order,
                     enabled = EXCLUDED.enabled,
                     updated_at = now(),
                     updated_by_id = EXCLUDED.updated_by_id,
                     updated_by_id_name = EXCLUDED.updated_by_id_name"#,
            )
            .bind(stage_id)
            .bind(idx as i32)
            .bind(enabled)
            .bind(ctx.operator_id)
            .bind(&ctx.operator_id_name)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }

        sqlx::query("DELETE FROM waf_pipeline_stages WHERE NOT (stage_id = ANY($1))")
            .bind(&order)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        let layout = serde_json::to_value(&cfg.pipeline_editor.layout)?;
        sqlx::query(
            r#"INSERT INTO waf_pipeline_editor
               (id, editor_mode, layout, graph, created_by_id, created_by_id_name, updated_by_id, updated_by_id_name)
               VALUES ($1, $2, $3, $4, $5, $6, $5, $6)
               ON CONFLICT (id) DO UPDATE SET
                 editor_mode = EXCLUDED.editor_mode,
                 layout = EXCLUDED.layout,
                 graph = EXCLUDED.graph,
                 updated_at = now(),
                 updated_by_id = EXCLUDED.updated_by_id,
                 updated_by_id_name = EXCLUDED.updated_by_id_name"#,
        )
        .bind(GLOBAL_ROW_ID)
        .bind(&cfg.pipeline_editor.mode)
        .bind(layout)
        .bind(&cfg.pipeline_editor.graph)
        .bind(ctx.operator_id)
        .bind(&ctx.operator_id_name)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let semantic_json = serde_json::to_value(&cfg)?;
        if let Some(triage) = triage_llm {
            sqlx::query(
                r#"INSERT INTO waf_ai_settings (site_id, semantic, triage_llm, updated_at)
                   VALUES ('default', $1, $2, now())
                   ON CONFLICT (site_id) DO UPDATE SET
                     semantic = EXCLUDED.semantic,
                     triage_llm = EXCLUDED.triage_llm,
                     updated_at = now()"#,
            )
            .bind(semantic_json)
            .bind(triage)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        } else {
            sqlx::query(
                r#"INSERT INTO waf_ai_settings (site_id, semantic, triage_llm, updated_at)
                   VALUES ('default', $1, '{}'::jsonb, now())
                   ON CONFLICT (site_id) DO UPDATE SET
                     semantic = EXCLUDED.semantic,
                     updated_at = now()"#,
            )
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

    fn pipeline_order(cfg: &SemanticConfig) -> Vec<String> {
        let mut cfg = cfg.clone();
        cfg.engines.normalize_pipeline();
        cfg.engines.pipeline_order
    }

    fn stages_from_rows(
        stage_rows: &[sqlx::postgres::PgRow],
    ) -> (Vec<String>, HashMap<String, bool>) {
        let mut pipeline_order = Vec::new();
        let mut pipeline_enabled = HashMap::new();
        if stage_rows.is_empty() {
            for id in DEFAULT_PIPELINE_ORDER {
                pipeline_order.push((*id).into());
                pipeline_enabled.insert((*id).into(), true);
            }
        } else {
            for s in stage_rows {
                let id: String = s.get("stage_id");
                pipeline_order.push(id.clone());
                pipeline_enabled.insert(id, s.get("enabled"));
            }
        }
        (pipeline_order, pipeline_enabled)
    }

    fn build_semantic_config(
        row: sqlx::postgres::PgRow,
        pipeline_order: &[String],
        pipeline_enabled: &HashMap<String, bool>,
        editor_row: Option<sqlx::postgres::PgRow>,
    ) -> Result<SemanticConfig, ConfigError> {
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

        Ok(SemanticConfig {
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
                pipeline_order: pipeline_order.to_vec(),
                pipeline_enabled: pipeline_enabled.clone(),
            },
            performance: serde_json::from_value(performance).unwrap_or_default(),
            auto_tune: serde_json::from_value(auto_tune).unwrap_or_default(),
            intent_enabled: row.get("intent_enabled"),
            pipeline_editor,
            expert_level: Some(ExpertLevel::parse(
                row.get::<String, _>("expert_level").as_str(),
            )),
        }
        .normalized())
    }
}
