use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_semantic::{
    apply_auto_tune, now_iso, ExpertLevel, SemanticConfig, SemanticFeedback, SemanticMode,
    SemanticStatsRecorder,
};
use nerve_waf_store::{ProtectionStrategyTemplate, TriageLlmConfig};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::triage_routes::resolve_event;
use crate::AppState;

pub fn ai_settings_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/semantic/config", get(get_semantic_config).put(put_semantic_config))
        .route(
            "/api/global/semantic/config",
            get(get_global_semantic_config).put(put_global_semantic_config),
        )
        .route(
            "/api/global/protection-levels",
            get(get_global_protection_levels).put(put_global_protection_levels),
        )
        .route(
            "/api/protection-levels",
            get(get_app_protection_levels).put(put_app_protection_levels),
        )
        .route(
            "/api/global/protection-strategy",
            get(get_global_protection_levels).put(put_global_protection_levels),
        )
        .route(
            "/api/protection-strategy",
            get(get_app_protection_levels).put(put_app_protection_levels),
        )
        .route(
            "/api/protection-strategies",
            get(list_protection_strategies).post(create_protection_strategy),
        )
        .route(
            "/api/protection-strategies/{id}",
            get(get_protection_strategy)
                .put(update_protection_strategy)
                .delete(delete_protection_strategy),
        )
        .route(
            "/api/protection-strategies/{id}/apply",
            post(apply_protection_strategy),
        )
        .route("/api/semantic/stats", get(get_semantic_stats))
        .route("/api/semantic/feedback", post(post_semantic_feedback))
        .route("/api/triage/llm-config", get(get_llm_config).put(put_llm_config))
        .route("/api/ai-settings", get(get_ai_settings))
}



#[derive(Deserialize)]

struct SiteQuery {

    #[serde(default = "default_site")]

    site_id: String,

}



fn default_site() -> String {

    "default".into()

}



fn resolve_site_id(q: &SiteQuery) -> &str {

    if q.site_id.trim().is_empty() {

        "default"

    } else {

        q.site_id.trim()

    }

}



async fn get_global_semantic_config(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let cfg = state
        .config
        .load_global_pipeline()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "scope": "global",
        "config": cfg,
        "backend": state.ai_settings.backend_name(),
    })))
}

async fn put_global_semantic_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<SemanticConfig>,
) -> Result<Json<Value>, StatusCode> {
    let cfg = cfg.normalized();
    state
        .config
        .save_global_pipeline(&cfg, "console")
        .await
        .map_err(|e| {
            tracing::error!("save global semantic config: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "scope": "global",
        "config": cfg,
        "backend": state.ai_settings.backend_name(),
    })))
}

#[derive(Deserialize)]
struct ProtectionLevelsBody {
    #[serde(default)]
    semantic_mode: Option<String>,
    /// Required globally; app side always writes this app (no longer inherits global).
    #[serde(default)]
    expert_level: Option<String>,
    /// Legacy field; ignored.
    #[serde(default)]
    expert_level_override: Option<bool>,
    /// Strategy-library template id bound to the app (app side only).
    #[serde(default)]
    bound_strategy_id: Option<String>,
}

fn protection_levels_json(cfg: &SemanticConfig, scope: &str, inherited_expert: bool) -> Value {
    json!({
        "scope": scope,
        "semantic_mode": match cfg.mode {
            SemanticMode::Strict => "strict",
            SemanticMode::Relaxed => "relaxed",
            SemanticMode::Standard => "standard",
        },
        "expert_level": cfg.expert_level.unwrap_or(ExpertLevel::Medium).as_str(),
        "expert_level_inherited": inherited_expert,
        "core": {
            "mode": "per_module",
            "hint": "核心防护请按模块设置灵敏度（Web 核心防护 / 应用「防护模块」）",
        },
        "custom": {
            "mode": "per_ruleset",
            "hint": "自定义检测请在规则集中分别配置",
        },
    })
}

async fn get_global_protection_levels(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let cfg = state
        .config
        .load_global_pipeline()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(protection_levels_json(&cfg, "global", false)))
}

async fn put_global_protection_levels(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProtectionLevelsBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut cfg = state
        .config
        .load_global_pipeline()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(mode) = body.semantic_mode.as_deref() {
        cfg.mode = match mode {
            "strict" => SemanticMode::Strict,
            "relaxed" => SemanticMode::Relaxed,
            _ => SemanticMode::Standard,
        };
    }
    if let Some(level) = body.expert_level.as_deref() {
        cfg.expert_level = Some(ExpertLevel::parse(level));
    } else if cfg.expert_level.is_none() {
        cfg.expert_level = Some(ExpertLevel::Medium);
    }
    let cfg = cfg.normalized();
    state
        .config
        .save_global_pipeline(&cfg, "console")
        .await
        .map_err(|e| {
            tracing::error!("save global protection levels: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "levels": protection_levels_json(&cfg, "global", false),
    })))
}

async fn get_app_protection_levels(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q).to_string();
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
    let mut cfg = state.ai_settings.semantic_config_for(&site_id);
    // App snapshot is independent: default expert tier persists as medium; no runtime follow of global.
    if cfg.expert_level.is_none() {
        cfg.expert_level = Some(ExpertLevel::Medium);
        let _ = state.ai_settings.set_semantic_for(&site_id, cfg.clone()).await;
    }
    let mut out = protection_levels_json(&cfg, "app", false);
    if let Some(obj) = out.as_object_mut() {
        obj.insert("site_id".into(), json!(site_id));
        let bound = state
            .config
            .get_app_bound_strategy(&site_id)
            .await
            .unwrap_or_else(|_| "tpl-standard".into());
        obj.insert("bound_strategy_id".into(), json!(bound));
    }
    Ok(Json(out))
}

async fn put_app_protection_levels(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
    Json(body): Json<ProtectionLevelsBody>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q).to_string();
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
    let mut cfg = state.ai_settings.semantic_config_for(&site_id);
    let mut touched_semantic = false;
    if let Some(mode) = body.semantic_mode.as_deref() {
        cfg.mode = match mode {
            "strict" => SemanticMode::Strict,
            "relaxed" => SemanticMode::Relaxed,
            _ => SemanticMode::Standard,
        };
        touched_semantic = true;
    }
    if body.expert_level.is_some() || body.semantic_mode.is_some() {
        // App side always writes this app's expert tier; no 'inherit global'.
        let level = body
            .expert_level
            .as_deref()
            .map(ExpertLevel::parse)
            .or(cfg.expert_level)
            .unwrap_or(ExpertLevel::Medium);
        cfg.expert_level = Some(level);
        touched_semantic = true;
    }
    if touched_semantic {
        let cfg = cfg.normalized();
        state
            .ai_settings
            .set_semantic_for(&site_id, cfg)
            .await
            .map_err(|e| {
                tracing::error!("save app protection levels: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }
    if let Some(ref sid) = body.bound_strategy_id {
        let sid = sid.trim();
        if !sid.is_empty() {
            let tpl = state
                .config
                .get_protection_strategy(sid)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::BAD_REQUEST)?;
            if !tpl.visible_to_site(&site_id) {
                return Err(StatusCode::FORBIDDEN);
            }
            if !tpl.enabled {
                tracing::warn!("refuse bind disabled strategy {} to site {}", sid, site_id);
                return Err(StatusCode::BAD_REQUEST);
            }
            let _ = state
                .config
                .set_app_bound_strategy(&site_id, sid, "console")
                .await;
        }
    }
    let bound = state
        .config
        .get_app_bound_strategy(&site_id)
        .await
        .unwrap_or_else(|_| "tpl-standard".into());
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "site_id": site_id,
        "bound_strategy_id": bound,
    })))
}

fn template_json(t: &ProtectionStrategyTemplate) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "description": t.description,
        "semantic_mode": t.semantic_mode,
        "expert_level": t.expert_level,
        "builtin": t.builtin,
        "sort_order": t.sort_order,
        "site_id": t.site_id,
        "scope": if t.is_global() { "global" } else { "app" },
        "enabled": t.enabled,
        "expert": t.expert,
        "semantic": t.semantic,
        "core": t.core,
        "custom": t.custom,
    })
}

#[derive(Deserialize)]
struct StrategyListQuery {
    /// Omit: global strategies only (sidebar). With site_id: global + that app's private strategies.
    #[serde(default)]
    site_id: Option<String>,
}

#[derive(Deserialize)]
struct StrategyCreateBody {
    name: String,
    #[serde(default)]
    description: Option<String>,
    /// When set, write into app-private strategy table (visible only to that app).
    #[serde(default)]
    site_id: Option<String>,
    /// Optional: deep-copy four sections from that strategy; default copies current global runtime.
    #[serde(default)]
    source_id: Option<String>,
}

#[derive(Deserialize)]
struct StrategyUpdateBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    semantic_mode: Option<String>,
    #[serde(default)]
    expert_level: Option<String>,
    #[serde(default)]
    expert: Option<Value>,
    #[serde(default)]
    semantic: Option<Value>,
    #[serde(default)]
    core: Option<nerve_waf_sdk::ProtectionModules>,
    #[serde(default)]
    custom: Option<Value>,
    #[serde(default)]
    sort_order: Option<i32>,
    /// Only app-private strategies are editable; global ignored.
    #[serde(default)]
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct ApplyStrategyBody {
    /// global | app
    target: String,
    #[serde(default)]
    site_id: Option<String>,
}

async fn list_protection_strategies(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StrategyListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let for_site = q
        .site_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let list = state
        .config
        .list_protection_strategies(for_site)
        .await
        .map_err(|e| {
            tracing::error!("list protection strategies: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({
        "templates": list.iter().map(template_json).collect::<Vec<_>>(),
    })))
}

async fn get_protection_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let t = state
        .config
        .get_protection_strategy(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(template_json(&t)))
}

async fn create_protection_strategy(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StrategyCreateBody>,
) -> Result<Json<Value>, StatusCode> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let site_id = body
        .site_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let source_id = body
        .source_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(site) = site_id {
        state
            .config
            .ensure_app_provisioned(site)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let mut global_modules = state
        .config
        .load_global_modules()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cc = global_modules.merge_cc_config(&state.platform.cc.config());
    let bot = global_modules.merge_bot_config(&state.platform.bot_config());
    global_modules.materialize_basic_module_params(&cc, &bot);
    let saved = state
        .config
        .create_protection_strategy_from_global(
            name,
            body.description.as_deref().unwrap_or(""),
            site_id,
            source_id,
            Some(global_modules),
        )
        .await
        .map_err(|e| {
            tracing::warn!("create protection strategy: {e}");
            StatusCode::BAD_REQUEST
        })?;
    if site_id.is_some() && saved.is_global() {
        tracing::error!("create app strategy wrote to global table unexpectedly");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(Json(json!({ "ok": true, "template": template_json(&saved) })))
}

async fn update_protection_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<StrategyUpdateBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut existing = state
        .config
        .get_protection_strategy(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !existing.builtin {
        if let Some(n) = body.name {
            existing.name = n;
        }
    }
    if let Some(d) = body.description {
        existing.description = d;
    }
    if let Some(e) = body.expert {
        existing.expert = e;
    }
    if let Some(level) = body.expert_level {
        existing.expert_level = level.clone();
        existing.expert = json!({ "level": level });
    }
    if let Some(s) = body.semantic {
        existing.semantic = s;
    }
    if let Some(mode) = body.semantic_mode {
        existing.semantic_mode = mode.clone();
        if let Some(obj) = existing.semantic.as_object_mut() {
            obj.insert("mode".into(), json!(mode));
        } else {
            existing.semantic = json!({ "mode": mode });
        }
    }
    if let Some(c) = body.core {
        existing.core = c;
    }
    if let Some(c) = body.custom {
        existing.custom = c;
    }
    if let Some(o) = body.sort_order {
        existing.sort_order = o;
    }
    if let Some(en) = body.enabled {
        if existing.is_global() {
            existing.enabled = true;
        } else {
            existing.enabled = en;
        }
    }
    existing.sync_summary_fields();
    let saved = state
        .config
        .upsert_protection_strategy(existing)
        .await
        .map_err(|e| {
            tracing::warn!("update protection strategy: {e}");
            StatusCode::BAD_REQUEST
        })?;
    if !saved.enabled {
        let _ = state
            .config
            .rebind_sites_away_from_strategy(&saved.id, "console")
            .await;
    }
    Ok(Json(json!({ "ok": true, "template": template_json(&saved) })))
}

async fn delete_protection_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let ok = state
        .config
        .delete_protection_strategy(&id)
        .await
        .map_err(|e| {
            tracing::warn!("delete protection strategy: {e}");
            StatusCode::BAD_REQUEST
        })?;
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn apply_protection_strategy(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ApplyStrategyBody>,
) -> Result<Json<Value>, StatusCode> {
    match body.target.as_str() {
        "global" => {
            let existing = state
                .config
                .get_protection_strategy(&id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::NOT_FOUND)?;
            if !existing.is_global() {
                tracing::warn!("refuse apply app-private strategy to global: {}", id);
                return Err(StatusCode::BAD_REQUEST);
            }
            let tpl = state
                .config
                .apply_protection_strategy_global(&id, "console")
                .await
                .map_err(|e| {
                    tracing::error!("apply strategy to global: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            let cfg = tpl.to_pipeline_patch();
            // Refresh in-memory cache and L7 (DB already written to global tables by apply).
            let _ = state.ai_settings.refresh_site_from_store("default").await;
            state.refresh_l7_from_store().await;
            Ok(Json(json!({
                "ok": true,
                "target": "global",
                "template_id": tpl.id,
                "template_name": tpl.name,
                "levels": protection_levels_json(&cfg, "global", false),
            })))
        }
        "app" => {
            let tpl = state
                .config
                .get_protection_strategy(&id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::NOT_FOUND)?;
            let site_id = body
                .site_id
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or(StatusCode::BAD_REQUEST)?
                .to_string();
            if !tpl.visible_to_site(&site_id) {
                tracing::warn!(
                    "strategy {} not visible to site {}",
                    id,
                    site_id
                );
                return Err(StatusCode::FORBIDDEN);
            }
            if !tpl.enabled {
                tracing::warn!("refuse apply disabled strategy {} to site {}", id, site_id);
                return Err(StatusCode::BAD_REQUEST);
            }
            state
                .config
                .ensure_app_provisioned(&site_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
            let cfg = tpl.to_pipeline_patch();
            state
                .ai_settings
                .set_semantic_for(&site_id, cfg)
                .await
                .map_err(|e| {
                    tracing::error!("apply strategy semantic to app: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            let mut modules = tpl.core.clone();
            let cc = modules.merge_cc_config(&state.platform.cc.config());
            let bot = modules.merge_bot_config(&state.platform.bot_config());
            modules.materialize_basic_module_params(&cc, &bot);
            state
                .config
                .save_modules_for(&site_id, &modules, "console")
                .await
                .map_err(|e| {
                    tracing::error!("apply strategy modules to app: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            // App binding: app → this strategy (modules below belong to this app snapshot).
            let _ = state
                .config
                .set_app_bound_strategy(&site_id, &tpl.id, "console")
                .await;
            let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
            state.refresh_l7_from_store().await;
            crate::dataplane::bump_after_mutation(&state);
            Ok(Json(json!({
                "ok": true,
                "target": "app",
                "site_id": site_id,
                "template_id": tpl.id,
                "template_name": tpl.name,
                "bound_strategy_id": tpl.id,
            })))
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn get_semantic_config(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q).to_string();
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
    Ok(Json(json!({
        "site_id": site_id,
        "config": state.ai_settings.semantic_config_for(&site_id),
        "backend": state.ai_settings.backend_name(),
    })))
}



async fn put_semantic_config(

    State(state): State<Arc<AppState>>,

    Query(q): Query<SiteQuery>,

    Json(cfg): Json<SemanticConfig>,

) -> Result<Json<Value>, StatusCode> {

    let site_id = resolve_site_id(&q).to_string();
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cfg = cfg.normalized();

    state

        .ai_settings

        .set_semantic_for(&site_id, cfg.clone())

        .await

        .map_err(|e| {

            tracing::error!("save semantic config: {e}");

            StatusCode::INTERNAL_SERVER_ERROR

        })?;

    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({

        "ok": true,

        "site_id": site_id,

        "config": cfg,

        "backend": state.ai_settings.backend_name(),

    })))

}



async fn get_semantic_stats(

    State(state): State<Arc<AppState>>,

    Query(q): Query<SiteQuery>,

) -> Json<Value> {

    let site_id = resolve_site_id(&q);

    Json(json!(state.semantic_stats.as_ref().snapshot(site_id)))

}



#[derive(Deserialize)]

struct FeedbackPut {

    #[serde(default = "default_site")]

    site_id: String,

    event_id: Option<String>,

    verdict: String,

    note: Option<String>,

    param_path: Option<String>,

    uri: Option<String>,

}



async fn post_semantic_feedback(

    State(state): State<Arc<AppState>>,

    Json(body): Json<FeedbackPut>,

) -> Result<Json<Value>, StatusCode> {

    let site_id = if body.site_id.trim().is_empty() {

        "default".to_string()

    } else {

        body.site_id.trim().to_string()

    };



    let mut param_path = body.param_path;

    let mut uri = body.uri;

    if param_path.is_none() || uri.is_none() {

        if let Ok(ev) = resolve_event(&state, None, body.event_id.clone()).await {

            if param_path.is_none() {

                param_path = ev.message.as_ref().and_then(|m| {

                    m.split("param=")

                        .nth(1)

                        .map(|s| s.split_whitespace().next().unwrap_or("").to_string())

                        .filter(|s| !s.is_empty())

                });

            }

            if uri.is_none() {

                uri = ev.uri.clone();

            }

        }

    }



    let mut fb = SemanticFeedback {

        ts: now_iso(),

        site_id: site_id.clone(),

        event_id: body.event_id,

        verdict: body.verdict.clone(),

        note: body.note,

        param_path,

        uri,

        applied_action: None,

    };



    let mut auto_tune_result = None;

    let mut config_updated = false;

    let mut updated_config = None;



    if body.verdict == "false_positive" {

        let mut cfg = state.ai_settings.semantic_config_for(&site_id).normalized();

        let auto_tune = cfg.auto_tune.clone();

        let history = state.semantic_stats.as_ref().feedback_for_site(&site_id);

        let tune = apply_auto_tune(&mut cfg, &auto_tune, &history, &fb);

        if tune.config_changed {

            cfg = cfg.normalized();

            state

                .ai_settings

                .set_semantic_for(&site_id, cfg.clone())

                .await

                .map_err(|e| {

                    tracing::error!("auto-tune save: {e}");

                    StatusCode::INTERNAL_SERVER_ERROR

                })?;

            fb.applied_action = Some(tune.actions.join(";"));

            config_updated = true;

            updated_config = Some(cfg);

            auto_tune_result = Some(tune);

        }

    }



    state.semantic_stats.as_ref().add_feedback(fb.clone());

    let _ = state.semantic_stats.flush_dirty().await;



    Ok(Json(json!({

        "ok": true,

        "feedback": fb,

        "auto_tune": auto_tune_result,

        "config_updated": config_updated,

        "config": updated_config,

    })))

}



async fn get_llm_config(State(state): State<Arc<AppState>>) -> Json<Value> {

    let cfg = state.ai_settings.triage_llm_config();

    Json(json!({

        "config": redact_llm_config(&cfg),

        "backend": state.ai_settings.backend_name(),

    }))

}



async fn put_llm_config(

    State(state): State<Arc<AppState>>,

    Json(body): Json<LlmConfigPut>,

) -> Result<Json<Value>, StatusCode> {

    let mut cfg = state.ai_settings.triage_llm_config();

    if let Some(v) = body.enabled {

        cfg.enabled = v;

    }

    if let Some(v) = body.auto_explain {

        cfg.auto_explain = v;

    }

    if let Some(v) = body.api_url {

        cfg.api_url = v;

    }

    if let Some(v) = body.model {

        cfg.model = v;

    }

    if let Some(v) = body.api_key {

        if !v.is_empty() && v != "***" {

            cfg.api_key = v;

        }

    }

    state

        .ai_settings

        .set_triage_llm(cfg.clone())

        .await

        .map_err(|e| {

            tracing::error!("save llm config: {e}");

            StatusCode::INTERNAL_SERVER_ERROR

        })?;

    Ok(Json(json!({

        "ok": true,

        "config": redact_llm_config(&cfg),

        "backend": state.ai_settings.backend_name(),

    })))

}



async fn get_ai_settings(State(state): State<Arc<AppState>>) -> Json<Value> {

    Json(json!({

        "semantic": state.ai_settings.semantic_config(),

        "triage_llm": redact_llm_config(&state.ai_settings.triage_llm_config()),

        "backend": state.ai_settings.backend_name(),

    }))

}



#[derive(serde::Deserialize)]

struct LlmConfigPut {

    enabled: Option<bool>,

    auto_explain: Option<bool>,

    api_url: Option<String>,

    api_key: Option<String>,

    model: Option<String>,

}



fn redact_llm_config(cfg: &TriageLlmConfig) -> Value {

    json!({

        "enabled": cfg.enabled,

        "auto_explain": cfg.auto_explain,

        "api_url": cfg.api_url,

        "model": cfg.model,

        "api_key_set": !cfg.api_key.is_empty(),

        "llm_ready": cfg.llm_ready(),

    })

}


