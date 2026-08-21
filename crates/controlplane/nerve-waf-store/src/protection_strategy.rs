//! Protection strategy: strategy → four-section module snapshot (expert / core / semantic / custom).
//! Create strategy: deep-copy from current global runtime.

use nerve_waf_sdk::ProtectionModules;
use nerve_waf_semantic::{ExpertLevel, SemanticConfig, SemanticMode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::config::ConfigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionStrategyTemplate {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Compat list display; sync with expert.level / semantic.mode.
    #[serde(default = "default_mode_std")]
    pub semantic_mode: String,
    #[serde(default = "default_level_med")]
    pub expert_level: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub sort_order: i32,
    /// `None` = store in global table `waf_protection_strategies`;
    /// `Some(site)` = store in app-private `waf_app_protection_strategies` (visible only to that app).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_id: Option<String>,
    /// Only app-private strategies can be disabled; global/built-in stay true.
    #[serde(default = "default_enabled_true")]
    pub enabled: bool,
    /// `{ "level": "low|medium|high" }`
    #[serde(default = "default_expert_json")]
    pub expert: Value,
    /// Semantic snapshot: at least mode; may be full SemanticConfig.
    #[serde(default = "default_semantic_json")]
    pub semantic: Value,
    /// Full core-protection snapshot.
    #[serde(default)]
    pub core: ProtectionModules,
    /// Custom-detect snapshot (may be `{}` initially).
    #[serde(default = "default_custom_json")]
    pub custom: Value,
}

fn default_mode_std() -> String {
    "standard".into()
}
fn default_level_med() -> String {
    "medium".into()
}
fn default_enabled_true() -> bool {
    true
}
fn default_expert_json() -> Value {
    json!({ "level": "medium" })
}
fn default_semantic_json() -> Value {
    json!({ "mode": "standard" })
}
fn default_custom_json() -> Value {
    json!({})
}

impl ProtectionStrategyTemplate {
    pub fn is_global(&self) -> bool {
        self.site_id
            .as_deref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    }

    pub fn visible_to_site(&self, site_id: &str) -> bool {
        self.is_global()
            || self
                .site_id
                .as_deref()
                .map(|s| s == site_id)
                .unwrap_or(false)
    }

    /// Optional bind: must be visible to this app and not disabled.
    pub fn selectable_for_site(&self, site_id: &str) -> bool {
        self.enabled && self.visible_to_site(site_id)
    }

    pub fn sync_summary_fields(&mut self) {
        if let Some(level) = self.expert.get("level").and_then(|v| v.as_str()) {
            self.expert_level = normalize_level(level);
        } else {
            self.expert = json!({ "level": normalize_level(&self.expert_level) });
            self.expert_level = normalize_level(&self.expert_level);
        }
        if let Some(mode) = self.semantic.get("mode").and_then(|v| v.as_str()) {
            self.semantic_mode = normalize_mode(mode);
        } else if let Ok(cfg) = serde_json::from_value::<SemanticConfig>(self.semantic.clone()) {
            self.semantic_mode = match cfg.mode {
                SemanticMode::Strict => "strict".into(),
                SemanticMode::Relaxed => "relaxed".into(),
                SemanticMode::Standard => "standard".into(),
            };
        } else {
            self.semantic_mode = normalize_mode(&self.semantic_mode);
            if self.semantic.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                self.semantic = json!({ "mode": self.semantic_mode });
            }
        }
    }

    pub fn from_global_runtime(
        id: String,
        name: String,
        description: String,
        pipeline: &SemanticConfig,
        modules: &ProtectionModules,
    ) -> Self {
        let mode = match pipeline.mode {
            SemanticMode::Strict => "strict",
            SemanticMode::Relaxed => "relaxed",
            SemanticMode::Standard => "standard",
        };
        let level = pipeline
            .expert_level
            .unwrap_or(ExpertLevel::Medium)
            .as_str();
        let mut semantic = serde_json::to_value(pipeline).unwrap_or_else(|_| json!({ "mode": mode }));
        if let Some(obj) = semantic.as_object_mut() {
            obj.insert("mode".into(), json!(mode));
        }
        let mut t = Self {
            id,
            name,
            description,
            semantic_mode: mode.into(),
            expert_level: level.into(),
            builtin: false,
            sort_order: 100,
            site_id: None,
            enabled: true,
            expert: json!({ "level": level }),
            semantic,
            core: modules.clone(),
            custom: json!({}),
        };
        t.sync_summary_fields();
        t
    }

    pub fn apply_tier_defaults(mut self, tier: &str) -> Self {
        match tier {
            "relaxed" | "宽松" => {
                self.semantic_mode = "relaxed".into();
                self.expert_level = "low".into();
                self.expert = json!({ "level": "low" });
                bump_semantic_mode(&mut self.semantic, "relaxed");
                scale_core_sensitivity(&mut self.core, "low");
            }
            "strict" | "严格" => {
                self.semantic_mode = "strict".into();
                self.expert_level = "high".into();
                self.expert = json!({ "level": "high" });
                bump_semantic_mode(&mut self.semantic, "strict");
                scale_core_sensitivity(&mut self.core, "high");
            }
            _ => {
                self.semantic_mode = "standard".into();
                self.expert_level = "medium".into();
                self.expert = json!({ "level": "medium" });
                bump_semantic_mode(&mut self.semantic, "standard");
            }
        }
        self.sort_order = match tier {
            "relaxed" | "宽松" => 10,
            "strict" | "严格" => 30,
            _ => 20,
        };
        self.sync_summary_fields();
        self
    }

    pub fn to_pipeline_patch(&self) -> SemanticConfig {
        let mut cfg = serde_json::from_value::<SemanticConfig>(self.semantic.clone())
            .unwrap_or_default();
        cfg.mode = match self.semantic_mode.as_str() {
            "strict" => SemanticMode::Strict,
            "relaxed" => SemanticMode::Relaxed,
            _ => SemanticMode::Standard,
        };
        cfg.expert_level = Some(ExpertLevel::parse(&self.expert_level));
        cfg.normalized()
    }
}

fn bump_semantic_mode(semantic: &mut Value, mode: &str) {
    if let Some(obj) = semantic.as_object_mut() {
        obj.insert("mode".into(), json!(mode));
    } else {
        *semantic = json!({ "mode": mode });
    }
}

fn scale_core_sensitivity(core: &mut ProtectionModules, sensitivity: &str) {
    for cfg in core.modules.values_mut() {
        cfg.sensitivity = sensitivity.into();
    }
}

fn normalize_mode(s: &str) -> String {
    match s {
        "strict" | "relaxed" | "standard" => s.into(),
        _ => "standard".into(),
    }
}

fn normalize_level(s: &str) -> String {
    match s {
        "low" | "medium" | "high" => s.into(),
        _ => "medium".into(),
    }
}

pub fn normalize_template(mut t: ProtectionStrategyTemplate) -> ProtectionStrategyTemplate {
    t.name = t.name.trim().to_string();
    t.description = t.description.trim().to_string();
    if t.site_id.as_deref().map(|s| s.trim().is_empty()).unwrap_or(false) {
        t.site_id = None;
    }
    if t.builtin {
        t.site_id = None;
        t.enabled = true;
    }
    if t.is_global() {
        t.enabled = true;
    }
    t.sync_summary_fields();
    t
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StrategyFile {
    templates: Vec<ProtectionStrategyTemplate>,
}

fn builtin_from_runtime(
    modules: &ProtectionModules,
    pipeline: &SemanticConfig,
) -> Vec<ProtectionStrategyTemplate> {
    let base = || {
        ProtectionStrategyTemplate::from_global_runtime(
            "tmp".into(),
            "tmp".into(),
            String::new(),
            pipeline,
            modules,
        )
    };
    vec![
        {
            let mut t = base().apply_tier_defaults("relaxed");
            t.id = "tpl-relaxed".into();
            t.name = "宽松".into();
            t.description = "更少误报 · 适合开发联调".into();
            t.builtin = true;
            t
        },
        {
            let mut t = base().apply_tier_defaults("standard");
            t.id = "tpl-standard".into();
            t.name = "标准".into();
            t.description = "推荐默认 · 平衡检出与误报".into();
            t.builtin = true;
            t
        },
        {
            let mut t = base().apply_tier_defaults("strict");
            t.id = "tpl-strict".into();
            t.name = "严格".into();
            t.description = "更严拦截 · 高风险或合规场景".into();
            t.builtin = true;
            t
        },
    ]
}

fn file_path(data_dir: &Path) -> PathBuf {
    data_dir.join("protection_strategies.json")
}

fn read_file(data_dir: &Path) -> Result<StrategyFile, ConfigError> {
    let path = file_path(data_dir);
    if !path.exists() {
        return Ok(StrategyFile::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn write_file(data_dir: &Path, file: &StrategyFile) -> Result<(), ConfigError> {
    std::fs::create_dir_all(data_dir)?;
    let path = file_path(data_dir);
    let raw = serde_json::to_string_pretty(file)?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn migrate_legacy_ids(templates: &mut Vec<ProtectionStrategyTemplate>) -> bool {
    let mut changed = false;
    let map = [
        ("tpl-dev-relaxed", "tpl-relaxed", "宽松"),
        ("tpl-prod-standard", "tpl-standard", "标准"),
        ("tpl-prod-strict", "tpl-strict", "严格"),
    ];
    for (old, new, name) in map {
        if let Some(t) = templates.iter_mut().find(|t| t.id == old) {
            t.id = new.into();
            t.name = name.into();
            t.builtin = true;
            t.sync_summary_fields();
            changed = true;
        }
    }
    changed
}

pub fn ensure_seeds_file(
    data_dir: &Path,
    modules: &ProtectionModules,
    pipeline: &SemanticConfig,
) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
    let mut file = read_file(data_dir)?;
    let mut changed = false;
    if file.templates.is_empty() {
        file.templates = builtin_from_runtime(modules, pipeline);
        changed = true;
    } else if migrate_legacy_ids(&mut file.templates) {
        changed = true;
    }
    for t in &mut file.templates {
        let before = t.core.modules.is_empty();
        t.sync_summary_fields();
        if before {
            t.core = modules.clone();
            if t.semantic.as_object().map(|o| o.len() <= 1).unwrap_or(true) {
                let mut sem =
                    serde_json::to_value(pipeline).unwrap_or_else(|_| json!({ "mode": t.semantic_mode }));
                bump_semantic_mode(&mut sem, &t.semantic_mode);
                t.semantic = sem;
            }
            changed = true;
        }
    }
    if changed {
        write_file(data_dir, &file)?;
    }
    file.templates.sort_by_key(|t| t.sort_order);
    Ok(file.templates)
}

fn app_strategies_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("app_protection_strategies")
}

fn app_strategies_path(data_dir: &Path, site_id: &str) -> PathBuf {
    app_strategies_dir(data_dir).join(format!("{site_id}.json"))
}

fn read_app_file(data_dir: &Path, site_id: &str) -> Result<StrategyFile, ConfigError> {
    let path = app_strategies_path(data_dir, site_id);
    if !path.exists() {
        return Ok(StrategyFile::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn write_app_file(
    data_dir: &Path,
    site_id: &str,
    file: &StrategyFile,
) -> Result<(), ConfigError> {
    let dir = app_strategies_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let path = app_strategies_path(data_dir, site_id);
    let raw = serde_json::to_string_pretty(file)?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn list_all_app_templates(data_dir: &Path) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
    let dir = app_strategies_dir(data_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
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
        let raw = std::fs::read_to_string(&path)?;
        let file: StrategyFile = serde_json::from_str(&raw)?;
        for mut t in file.templates {
            t.site_id = Some(site.clone());
            t.builtin = false;
            t.sync_summary_fields();
            out.push(t);
        }
    }
    Ok(out)
}

pub fn list_file(
    data_dir: &Path,
    modules: &ProtectionModules,
    pipeline: &SemanticConfig,
    for_site: Option<&str>,
) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
    let mut global = ensure_seeds_file(data_dir, modules, pipeline)?;
    // Global file keeps only global strategies (prevent historical site_id mix-in).
    global.retain(|t| t.is_global());
    match for_site.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => Ok(global),
        Some(site) => {
            let mut app = read_app_file(data_dir, site)?.templates;
            for t in &mut app {
                t.site_id = Some(site.to_string());
                t.builtin = false;
                t.sync_summary_fields();
            }
            global.extend(app);
            global.sort_by(|a, b| {
                a.sort_order
                    .cmp(&b.sort_order)
                    .then_with(|| a.name.cmp(&b.name))
            });
            Ok(global)
        }
    }
}

pub fn get_file(
    data_dir: &Path,
    id: &str,
    modules: &ProtectionModules,
    pipeline: &SemanticConfig,
) -> Result<Option<ProtectionStrategyTemplate>, ConfigError> {
    if let Some(t) = ensure_seeds_file(data_dir, modules, pipeline)?
        .into_iter()
        .find(|t| t.id == id && t.is_global())
    {
        return Ok(Some(t));
    }
    Ok(list_all_app_templates(data_dir)?
        .into_iter()
        .find(|t| t.id == id))
}

pub fn upsert_file(
    data_dir: &Path,
    template: ProtectionStrategyTemplate,
    modules: &ProtectionModules,
    pipeline: &SemanticConfig,
) -> Result<ProtectionStrategyTemplate, ConfigError> {
    let mut t = normalize_template(template);
    if t.name.is_empty() {
        return Err(ConfigError::Db("策略名称不能为空".into()));
    }
    if let Some(site) = t.site_id.clone().filter(|s| !s.is_empty()) {
        // App-private: separate file; do not write global library.
        t.builtin = false;
        t.site_id = Some(site.clone());
        let mut file = read_app_file(data_dir, &site)?;
        if file.templates.iter().any(|x| x.name == t.name && x.id != t.id) {
            return Err(ConfigError::Db(format!("策略名称已存在: {}", t.name)));
        }
        if let Some(slot) = file.templates.iter_mut().find(|x| x.id == t.id) {
            *slot = t.clone();
        } else {
            file.templates.push(t.clone());
        }
        write_app_file(data_dir, &site, &file)?;
        return Ok(t);
    }

    // Global strategy library.
    t.site_id = None;
    let mut file = read_file(data_dir)?;
    if file.templates.is_empty() {
        file.templates = builtin_from_runtime(modules, pipeline);
    }
    // Clean private strategies mistakenly written into the global file.
    file.templates.retain(|x| x.is_global());
    if file
        .templates
        .iter()
        .any(|x| x.name == t.name && x.id != t.id)
    {
        return Err(ConfigError::Db(format!("策略名称已存在: {}", t.name)));
    }
    if let Some(slot) = file.templates.iter_mut().find(|x| x.id == t.id) {
        let builtin = slot.builtin;
        let name = if builtin {
            slot.name.clone()
        } else {
            t.name.clone()
        };
        *slot = t.clone();
        slot.builtin = builtin;
        slot.name = name;
        slot.site_id = None;
    } else {
        file.templates.push(t.clone());
    }
    write_file(data_dir, &file)?;
    Ok(t)
}

pub fn delete_file(data_dir: &Path, id: &str) -> Result<bool, ConfigError> {
    let mut file = read_file(data_dir)?;
    if let Some(idx) = file.templates.iter().position(|t| t.id == id) {
        if file.templates[idx].builtin {
            return Err(ConfigError::Db("内置策略不可删除".into()));
        }
        file.templates.remove(idx);
        write_file(data_dir, &file)?;
        return Ok(true);
    }
    let dir = app_strategies_dir(data_dir);
    if !dir.exists() {
        return Ok(false);
    }
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
        let mut app = read_app_file(data_dir, &site)?;
        let Some(idx) = app.templates.iter().position(|t| t.id == id) else {
            continue;
        };
        app.templates.remove(idx);
        write_app_file(data_dir, &site, &app)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(feature = "postgres")]
pub mod pg {
    use super::*;
    use sqlx::{PgPool, Row};
    use tokio::sync::OnceCell;

    /// Seed data runs once in-process. Schema assumed created by startup migrations; no DDL on business paths.
    static SEEDS_READY: OnceCell<()> = OnceCell::const_new();

    fn row_to_global(row: &sqlx::postgres::PgRow) -> Result<ProtectionStrategyTemplate, ConfigError> {
        let core_val: Value = row.try_get("core").unwrap_or_else(|_| json!({}));
        let core: ProtectionModules = if core_val.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            ProtectionModules::default()
        } else {
            serde_json::from_value(core_val).unwrap_or_default()
        };
        let mut t = ProtectionStrategyTemplate {
            id: row.get("id"),
            name: row.get("name"),
            description: row.try_get("description").unwrap_or_default(),
            semantic_mode: row
                .try_get::<String, _>("semantic_mode")
                .unwrap_or_else(|_| "standard".into()),
            expert_level: row
                .try_get::<String, _>("expert_level")
                .unwrap_or_else(|_| "medium".into()),
            builtin: row.try_get("builtin").unwrap_or(false),
            sort_order: row.try_get("sort_order").unwrap_or(100),
            site_id: None,
            enabled: true,
            expert: row
                .try_get("expert")
                .unwrap_or_else(|_| json!({ "level": "medium" })),
            semantic: row
                .try_get("semantic")
                .unwrap_or_else(|_| json!({ "mode": "standard" })),
            core,
            custom: row.try_get("custom").unwrap_or_else(|_| json!({})),
        };
        t.sync_summary_fields();
        Ok(t)
    }

    fn row_to_app(row: &sqlx::postgres::PgRow) -> Result<ProtectionStrategyTemplate, ConfigError> {
        let mut t = row_to_global(row)?;
        t.builtin = false;
        t.site_id = Some(row.get::<String, _>("site_id"));
        t.enabled = row.try_get("enabled").unwrap_or(true);
        t.sync_summary_fields();
        Ok(t)
    }

    /// Whether in-process seed is ready (after ready, list/get/upsert no longer load global modules/pipeline).
    pub fn is_ready() -> bool {
        SEEDS_READY.get().is_some()
    }

    /// Mark ready when list already read data; avoid OnceCell seed path (prevent init hang stalling all requests).
    pub fn mark_ready() {
        let _ = SEEDS_READY.set(());
    }

    pub async fn ensure_seeds(
        pool: &PgPool,
        modules: &ProtectionModules,
        pipeline: &SemanticConfig,
    ) -> Result<(), ConfigError> {
        if SEEDS_READY.get().is_some() {
            return Ok(());
        }
        // Do not use get_or_try_init: if init future hangs, all waiters stay stuck forever.
        // Concurrent duplicate runs are OK (insert/update idempotent); add timeout so API is not stalled.
        match tokio::time::timeout(
            std::time::Duration::from_secs(8),
            run_ensure_seeds(pool, modules, pipeline),
        )
        .await
        {
            Ok(Ok(())) => {
                mark_ready();
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ConfigError::Db(
                "protection strategy seed timed out".into(),
            )),
        }
    }

    async fn run_ensure_seeds(
        pool: &PgPool,
        modules: &ProtectionModules,
        pipeline: &SemanticConfig,
    ) -> Result<(), ConfigError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waf_protection_strategies")
            .fetch_one(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        if count == 0 {
            for t in builtin_from_runtime(modules, pipeline) {
                insert_global(pool, &t).await?;
            }
            return Ok(());
        }
        for (old, new, name) in [
            ("tpl-dev-relaxed", "tpl-relaxed", "宽松"),
            ("tpl-prod-standard", "tpl-standard", "标准"),
            ("tpl-prod-strict", "tpl-strict", "严格"),
        ] {
            let _ = sqlx::query(
                r#"UPDATE waf_protection_strategies
                   SET id = $2, name = $3, builtin = TRUE, updated_at = now()
                   WHERE id = $1"#,
            )
            .bind(old)
            .bind(new)
            .bind(name)
            .execute(pool)
            .await;
        }
        let need_backfill: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM waf_protection_strategies
               WHERE core = '{}'::jsonb OR core IS NULL"#,
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if need_backfill == 0 {
            return Ok(());
        }
        let core_json = serde_json::to_value(modules).unwrap_or(json!({}));
        let sem_json = serde_json::to_value(pipeline).unwrap_or(json!({}));
        let _ = sqlx::query(
            r#"UPDATE waf_protection_strategies
               SET core = $1, semantic = CASE
                 WHEN semantic = '{}'::jsonb OR semantic = '{"mode":"standard"}'::jsonb
                   THEN $2 || jsonb_build_object('mode', COALESCE(semantic_mode, 'standard'))
                 ELSE semantic
               END,
               expert = jsonb_build_object('level', COALESCE(expert_level, 'medium'))
               WHERE core = '{}'::jsonb OR core IS NULL"#,
        )
        .bind(&core_json)
        .bind(&sem_json)
        .execute(pool)
        .await;
        Ok(())
    }

    async fn insert_global(
        pool: &PgPool,
        t: &ProtectionStrategyTemplate,
    ) -> Result<(), ConfigError> {
        let core = serde_json::to_value(&t.core).unwrap_or(json!({}));
        sqlx::query(
            r#"INSERT INTO waf_protection_strategies
               (id, name, description, semantic_mode, expert_level, builtin, sort_order,
                expert, semantic, core, custom)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(&t.id)
        .bind(&t.name)
        .bind(&t.description)
        .bind(&t.semantic_mode)
        .bind(&t.expert_level)
        .bind(t.builtin)
        .bind(t.sort_order)
        .bind(&t.expert)
        .bind(&t.semantic)
        .bind(&core)
        .bind(&t.custom)
        .execute(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    async fn list_global(pool: &PgPool) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT id, name, description, semantic_mode, expert_level, builtin, sort_order,
                      expert, semantic, core, custom
               FROM waf_protection_strategies ORDER BY sort_order, name"#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        rows.iter().map(row_to_global).collect()
    }

    async fn list_app_for_site(
        pool: &PgPool,
        site_id: &str,
    ) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT id, site_id, name, description, semantic_mode, expert_level, sort_order,
                      enabled, expert, semantic, core, custom
               FROM waf_app_protection_strategies
               WHERE site_id = $1
               ORDER BY sort_order, name"#,
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        rows.iter().map(row_to_app).collect()
    }

    pub async fn list(
        pool: &PgPool,
        modules: &ProtectionModules,
        pipeline: &SemanticConfig,
        for_site: Option<&str>,
    ) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
        ensure_seeds(pool, modules, pipeline).await?;
        list_ready(pool, for_site).await
    }

    /// Assume seed ready: SELECT only (hot path; avoid loading global modules every time).
    pub async fn list_ready(
        pool: &PgPool,
        for_site: Option<&str>,
    ) -> Result<Vec<ProtectionStrategyTemplate>, ConfigError> {
        let mut global = list_global(pool).await?;
        match for_site.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => Ok(global),
            Some(site) => {
                let app = list_app_for_site(pool, site).await?;
                global.extend(app);
                global.sort_by(|a, b| {
                    a.sort_order
                        .cmp(&b.sort_order)
                        .then_with(|| a.name.cmp(&b.name))
                });
                Ok(global)
            }
        }
    }

    pub async fn get(
        pool: &PgPool,
        id: &str,
        modules: &ProtectionModules,
        pipeline: &SemanticConfig,
    ) -> Result<Option<ProtectionStrategyTemplate>, ConfigError> {
        ensure_seeds(pool, modules, pipeline).await?;
        get_ready(pool, id).await
    }

    pub async fn get_ready(
        pool: &PgPool,
        id: &str,
    ) -> Result<Option<ProtectionStrategyTemplate>, ConfigError> {
        let row = sqlx::query(
            r#"SELECT id, name, description, semantic_mode, expert_level, builtin, sort_order,
                      expert, semantic, core, custom
               FROM waf_protection_strategies WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        if let Some(r) = row.as_ref() {
            return Ok(Some(row_to_global(r)?));
        }
        let row = sqlx::query(
            r#"SELECT id, site_id, name, description, semantic_mode, expert_level, sort_order,
                      enabled, expert, semantic, core, custom
               FROM waf_app_protection_strategies WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        row.as_ref().map(row_to_app).transpose()
    }

    async fn upsert_global(
        pool: &PgPool,
        t: &ProtectionStrategyTemplate,
    ) -> Result<(), ConfigError> {
        let core = serde_json::to_value(&t.core).unwrap_or(json!({}));
        sqlx::query(
            r#"INSERT INTO waf_protection_strategies
               (id, name, description, semantic_mode, expert_level, builtin, sort_order,
                expert, semantic, core, custom, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11, now())
               ON CONFLICT (id) DO UPDATE SET
                 name = EXCLUDED.name,
                 description = EXCLUDED.description,
                 semantic_mode = EXCLUDED.semantic_mode,
                 expert_level = EXCLUDED.expert_level,
                 sort_order = EXCLUDED.sort_order,
                 expert = EXCLUDED.expert,
                 semantic = EXCLUDED.semantic,
                 core = EXCLUDED.core,
                 custom = EXCLUDED.custom,
                 updated_at = now()"#,
        )
        .bind(&t.id)
        .bind(&t.name)
        .bind(&t.description)
        .bind(&t.semantic_mode)
        .bind(&t.expert_level)
        .bind(t.builtin)
        .bind(t.sort_order)
        .bind(&t.expert)
        .bind(&t.semantic)
        .bind(&core)
        .bind(&t.custom)
        .execute(pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("idx_waf_protection_strategies_name") {
                ConfigError::Db(format!("策略名称已存在: {}", t.name))
            } else {
                ConfigError::Db(msg)
            }
        })?;
        Ok(())
    }

    async fn upsert_app(
        pool: &PgPool,
        t: &ProtectionStrategyTemplate,
        site_id: &str,
    ) -> Result<(), ConfigError> {
        let core = serde_json::to_value(&t.core).unwrap_or(json!({}));
        sqlx::query(
            r#"INSERT INTO waf_app_protection_strategies
               (id, site_id, name, description, semantic_mode, expert_level, sort_order,
                enabled, expert, semantic, core, custom, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12, now())
               ON CONFLICT (id) DO UPDATE SET
                 name = EXCLUDED.name,
                 description = EXCLUDED.description,
                 semantic_mode = EXCLUDED.semantic_mode,
                 expert_level = EXCLUDED.expert_level,
                 sort_order = EXCLUDED.sort_order,
                 enabled = EXCLUDED.enabled,
                 expert = EXCLUDED.expert,
                 semantic = EXCLUDED.semantic,
                 core = EXCLUDED.core,
                 custom = EXCLUDED.custom,
                 updated_at = now()
               WHERE waf_app_protection_strategies.site_id = EXCLUDED.site_id"#,
        )
        .bind(&t.id)
        .bind(site_id)
        .bind(&t.name)
        .bind(&t.description)
        .bind(&t.semantic_mode)
        .bind(&t.expert_level)
        .bind(t.sort_order)
        .bind(t.enabled)
        .bind(&t.expert)
        .bind(&t.semantic)
        .bind(&core)
        .bind(&t.custom)
        .execute(pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("idx_waf_app_protection_strategies") {
                ConfigError::Db(format!("策略名称已存在: {}", t.name))
            } else {
                ConfigError::Db(msg)
            }
        })?;
        Ok(())
    }

    pub async fn upsert(
        pool: &PgPool,
        template: ProtectionStrategyTemplate,
        modules: &ProtectionModules,
        pipeline: &SemanticConfig,
    ) -> Result<ProtectionStrategyTemplate, ConfigError> {
        ensure_seeds(pool, modules, pipeline).await?;
        upsert_ready(pool, template).await
    }

    pub async fn upsert_ready(
        pool: &PgPool,
        template: ProtectionStrategyTemplate,
    ) -> Result<ProtectionStrategyTemplate, ConfigError> {
        let mut t = normalize_template(template);
        if t.name.is_empty() {
            return Err(ConfigError::Db("策略名称不能为空".into()));
        }
        if let Some(existing) = get_ready(pool, &t.id).await? {
            if existing.is_global() {
                t.site_id = None;
                if existing.builtin {
                    t.name = existing.name;
                    t.builtin = true;
                }
            } else {
                t.site_id = existing.site_id;
                t.builtin = false;
            }
        }
        if let Some(site) = t.site_id.clone().filter(|s| !s.is_empty()) {
            t.builtin = false;
            t.site_id = Some(site.clone());
            upsert_app(pool, &t, &site).await?;
        } else {
            t.site_id = None;
            upsert_global(pool, &t).await?;
        }
        Ok(t)
    }

    pub async fn delete(pool: &PgPool, id: &str) -> Result<bool, ConfigError> {
        delete_ready(pool, id).await
    }

    pub async fn delete_ready(pool: &PgPool, id: &str) -> Result<bool, ConfigError> {
        let builtin: Option<bool> =
            sqlx::query_scalar("SELECT builtin FROM waf_protection_strategies WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
        if let Some(b) = builtin {
            if b {
                return Err(ConfigError::Db("内置策略不可删除".into()));
            }
            let r = sqlx::query("DELETE FROM waf_protection_strategies WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| ConfigError::Db(e.to_string()))?;
            return Ok(r.rows_affected() > 0);
        }
        let r = sqlx::query("DELETE FROM waf_app_protection_strategies WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(r.rows_affected() > 0)
    }
}
