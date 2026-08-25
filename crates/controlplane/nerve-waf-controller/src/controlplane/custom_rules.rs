use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use nerve_waf_l7::{CustomRule, CustomRuleSet, CustomRulesFile};
use nerve_waf_store::{new_rule_id, new_set_id};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn custom_rules_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/global/custom-rules",
            get(list_global_custom_rules).put(save_global_custom_rules),
        )
        .route("/api/global/custom-rules/reload", post(reload_global_custom_rules))
        .route("/api/global/custom-rules/sets", post(create_global_rule_set))
        .route("/api/global/custom-rules/sets/{id}/bind", put(bind_global_rule_set))
        .route(
            "/api/global/custom-rules/sets/{id}",
            put(update_global_rule_set).delete(delete_global_rule_set),
        )
        .route("/api/global/custom-rules/rules", post(create_global_rule))
        .route(
            "/api/global/custom-rules/rules/{id}",
            put(update_global_rule).delete(delete_global_rule),
        )
        .route("/api/global/custom-rules/test", post(test_custom_rule))
        .route("/api/custom-rules", get(list_app_custom_rules).put(save_app_custom_rules))
        .route("/api/custom-rules/test", post(test_custom_rule))
}

#[derive(Deserialize)]
struct SiteQuery {
    #[serde(default)]
    site_id: String,
}

fn resolve_site_id(q: &SiteQuery) -> String {
    let id = q.site_id.trim();
    if id.is_empty() {
        "default".into()
    } else {
        id.to_string()
    }
}

async fn list_global_custom_rules(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "scope": "global",
        "sets": data.sets,
        "rules": data.rules,
    })))
}

async fn save_global_custom_rules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CustomRulesFile>,
) -> Result<Json<Value>, StatusCode> {
    state
        .custom_rules_store
        .save_global(&body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "scope": "global" })))
}

async fn reload_global_custom_rules(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let rules = state
        .custom_rules_store
        .load_active_rules_for("default")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    *state.custom_rules_cache.write().unwrap() = rules.clone();
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "activeRules": rules.len() })))
}

async fn list_app_custom_rules(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q);
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let data = state
        .custom_rules_store
        .load_all_for(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "site_id": site_id,
        "sets": data.sets,
        "rules": data.rules,
    })))
}

async fn save_app_custom_rules(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
    Json(body): Json<CustomRulesFile>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q);
    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .custom_rules_store
        .save_all_for(&site_id, &body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if site_id == "default" {
        state.reload_custom_rules_cache().await;
    }
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "site_id": site_id })))
}

#[derive(Deserialize)]
struct RuleSetBody {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

async fn create_global_rule_set(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RuleSetBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let set = CustomRuleSet {
        id: new_set_id(),
        name: body.name,
        description: body.description,
        enabled: body.enabled,
    };
    data.sets.push(set.clone());
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "set": set })))
}

async fn update_global_rule_set(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RuleSetBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(set) = data.sets.iter_mut().find(|s| s.id == id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    set.name = body.name;
    set.description = body.description;
    set.enabled = body.enabled;
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn delete_global_rule_set(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    data.sets.retain(|s| s.id != id);
    for r in &mut data.rules {
        if r.rule_set_id == id {
            r.rule_set_id.clear();
        }
    }
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct BindRuleSetBody {
    rule_ids: Vec<String>,
}

async fn bind_global_rule_set(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<BindRuleSetBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !data.sets.iter().any(|s| s.id == id) {
        return Err(StatusCode::NOT_FOUND);
    }
    let want: std::collections::HashSet<_> = body.rule_ids.iter().map(String::as_str).collect();
    for r in &mut data.rules {
        if r.rule_set_id == id && !want.contains(r.id.as_str()) {
            r.rule_set_id.clear();
        }
    }
    for rid in &body.rule_ids {
        if let Some(r) = data.rules.iter_mut().find(|r| r.id == *rid) {
            r.rule_set_id = id.clone();
        }
    }
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "bound": body.rule_ids.len() })))
}

#[derive(Deserialize)]
struct RuleBody {
    #[serde(default)]
    rule_set_id: String,
    name: String,
    #[serde(default = "default_rule_type_field")]
    rule_type: String,
    #[serde(default)]
    plugin_name: String,
    #[serde(default = "default_phase")]
    phase: String,
    #[serde(default = "default_priority")]
    priority: i32,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    match_field: String,
    #[serde(default)]
    match_op: String,
    #[serde(default)]
    match_pattern: String,
    #[serde(default = "default_action")]
    action: String,
    #[serde(default = "default_severity")]
    severity: String,
    #[serde(default = "default_category")]
    category: String,
}

fn default_rule_type_field() -> String {
    "signature".into()
}

fn default_phase() -> String {
    "request".into()
}
fn default_priority() -> i32 {
    100
}
fn default_action() -> String {
    "block".into()
}
fn default_severity() -> String {
    "medium".into()
}
fn default_category() -> String {
    "custom".into()
}

fn rule_from_body(id: Option<String>, body: &RuleBody) -> CustomRule {
    CustomRule {
        id: id.unwrap_or_else(new_rule_id),
        rule_set_id: body.rule_set_id.clone(),
        name: body.name.clone(),
        phase: body.phase.clone(),
        priority: body.priority,
        enabled: body.enabled,
        match_field: body.match_field.clone(),
        match_op: body.match_op.clone(),
        match_pattern: body.match_pattern.clone(),
        action: body.action.clone(),
        severity: body.severity.clone(),
        category: body.category.clone(),
        rule_type: if body.rule_type.is_empty() {
            "signature".into()
        } else {
            body.rule_type.clone()
        },
        plugin_name: body.plugin_name.clone(),
    }
}

async fn create_global_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RuleBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !body.rule_set_id.is_empty() && !data.sets.iter().any(|s| s.id == body.rule_set_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let rule = rule_from_body(None, &body);
    data.rules.push(rule.clone());
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "rule": rule })))
}

async fn update_global_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RuleBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(rule) = data.rules.iter_mut().find(|r| r.id == id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    *rule = rule_from_body(Some(id), &body);
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn delete_global_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut data = state
        .custom_rules_store
        .load_global()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    data.rules.retain(|r| r.id != id);
    state
        .custom_rules_store
        .save_global(&data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TestRuleBody {
    #[serde(flatten)]
    rule: RuleBody,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    headers: String,
    #[serde(default)]
    body: String,
}

async fn test_custom_rule(Json(body): Json<TestRuleBody>) -> Json<Value> {
    use nerve_waf_l7::InspectRequest;
    let rule = rule_from_body(Some("test".into()), &body.rule);
    let req = InspectRequest {
        method: "GET".into(),
        host: "test".into(),
        uri: body.uri,
        headers: body.headers,
        body: body.body,
        src_ip: "127.0.0.1".into(),
        user_agent: "test".into(),
    };
    let hit = nerve_waf_l7::inspect_custom_rules(&[rule], &req, "block");
    Json(json!({ "hit": hit }))
}
