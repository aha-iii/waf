use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use nerve_waf_bot::{score as bot_score, BotConfig};
use nerve_waf_behavior::BehaviorConfig;
use nerve_waf_intel::IntelEntry;
use nerve_waf_sdk::WafEventRecord;
use nerve_waf_auth::{AuthConfigResponse, LoginRequest, LoginResponse};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn extensions_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/bot/score", post(api_bot_score))
        .route("/api/bot/config", get(get_bot_config).put(put_bot_config))
        .route("/api/behavior/config", get(get_behavior_config).put(put_behavior_config))
        .route("/api/behavior/score", post(behavior_score))
        .route("/api/intel/feed", get(get_intel_feed).put(put_intel_feed))
        .route("/api/intel/lookup", post(intel_lookup))
        .route("/api/governance/holdings", get(governance_holdings))
        .route("/api/playbook/block-ip", post(playbook_block_ip))
        .route("/api/compliance/report", get(compliance_report))
        .route("/api/export/siem", post(export_siem))
        .route("/api/auth/status", get(auth_status))
        .route("/api/auth/config", get(auth_config))
        .route("/api/auth/login", post(auth_login))
}

#[derive(Deserialize)]
struct BotScoreBody {
    user_agent: String,
    uri: String,
}

async fn api_bot_score(Json(body): Json<BotScoreBody>) -> Json<Value> {
    let v = bot_score(&body.user_agent, &body.uri);
    Json(json!({
        "score": v.score,
        "is_bot": v.is_bot,
        "reason": v.reason
    }))
}

async fn get_bot_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.bot_config() }))
}

async fn put_bot_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<BotConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_bot_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn get_behavior_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.behavior_config() }))
}

async fn put_behavior_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<BehaviorConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_behavior_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct BehaviorScoreBody {
    src_ip: String,
    method: String,
    uri: String,
    #[serde(default)]
    user_agent: String,
}

async fn behavior_score(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BehaviorScoreBody>,
) -> Json<Value> {
    let engine = state.platform.behavior.read().unwrap();
    let v = engine.score(&body.src_ip, &body.method, &body.uri, &body.user_agent);
    Json(json!({
        "score": v.score,
        "is_anomaly": v.is_anomaly,
        "reason": v.reason,
        "request_count": v.request_count,
        "unique_paths": v.unique_paths,
    }))
}

async fn get_intel_feed(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "entries": state.platform.intel_entries() }))
}

async fn put_intel_feed(
    State(state): State<Arc<AppState>>,
    Json(entries): Json<Vec<IntelEntry>>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_intel_feed(entries)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct IntelLookupBody {
    ip: String,
}

async fn intel_lookup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IntelLookupBody>,
) -> Json<Value> {
    let feed = state.platform.intel_feed();
    Json(json!({
        "ip": body.ip,
        "malicious": feed.is_malicious(&body.ip),
        "category": feed.category(&body.ip)
    }))
}

async fn governance_holdings(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "service_policies": state.platform.service_policies(),
        "east_west_l7": state.platform.east_west_policies(),
    }))
}

#[derive(Deserialize)]
struct BlockIpBody {
    ip: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn playbook_block_ip(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BlockIpBody>,
) -> Result<Json<Value>, StatusCode> {
    let plan = nerve_waf_sdk::SyncPlan {
        actions: vec![nerve_waf_sdk::PluginAction::DenyIp {
            ip: body.ip.clone(),
            reason: body.reason.clone(),
        }],
        routes_applied: 0,
        plugins_applied: 1,
        l7_bundle: None,
    };
    let result = nerve_waf_engine::sync_to_nerve(&state.nerve, &plan).await;
    if !result.ok {
        return Err(StatusCode::BAD_GATEWAY);
    }
    Ok(Json(json!({
        "ok": result.ok,
        "ip": body.ip,
        "applied": result.applied,
        "errors": result.errors
    })))
}

async fn compliance_report(State(state): State<Arc<AppState>>) -> Json<Value> {
    let policies = state.platform.service_policies();
    let sites = state.platform.list_sites();
    let cc = state.platform.cc.config();
    Json(json!({
        "generated_at": Utc::now(),
        "summary": {
            "gateway_sites": sites.len(),
            "service_policies": policies.len(),
            "cc_enabled": cc.enabled,
            "geo_enabled": state.platform.geo.read().map(|g| g.config().enabled).unwrap_or(false),
        },
        "checks": [
            { "id": "CC-01", "name": "CC 限流", "status": if cc.enabled { "pass" } else { "warn" }, "detail": "CC engine configured" },
            { "id": "GEO-01", "name": "Geo 策略", "status": "info", "detail": "Geo rules loaded" },
            { "id": "NP-01", "name": "服务级策略", "status": if policies.is_empty() { "warn" } else { "pass" }, "detail": format!("{} policies", policies.len()) },
        ]
    }))
}

#[derive(Deserialize)]
struct SiemExportBody {
    #[serde(default)]
    events: Vec<WafEventRecord>,
    #[serde(default = "default_siem_format")]
    format: String,
}

fn default_siem_format() -> String {
    "json".into()
}

async fn export_siem(Json(body): Json<SiemExportBody>) -> Json<Value> {
    let lines: Vec<Value> = body
        .events
        .iter()
        .map(|e| {
            json!({
                "vendor": "nerve",
                "product": "waf",
                "version": "1.0",
                "event": e
            })
        })
        .collect();
    Json(json!({ "format": body.format, "records": lines }))
}

async fn auth_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.auth.auth_config_response();
    Json(json!({
        "enabled": cfg.enabled,
        "method": state.auth.auth_method_label(),
        "mode": cfg.mode,
        "local": cfg.local,
        "keycloak": cfg.keycloak,
    }))
}

async fn auth_config(State(state): State<Arc<AppState>>) -> Json<AuthConfigResponse> {
    Json(state.auth.auth_config_response())
}

fn env_local_credentials() -> (String, String) {
    let user = std::env::var("NERVE_AUTH_USER").unwrap_or_else(|_| "admin".into());
    let pass = std::env::var("NERVE_AUTH_PASSWORD").unwrap_or_else(|_| "admin".into());
    (user.trim().to_string(), pass)
}

fn verify_env_login(username: &str, password: &str) -> bool {
    let (env_user, env_pass) = env_local_credentials();
    username == env_user && password == env_pass
}

async fn auth_login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    if !state.auth.local_config().enabled {
        return Err(StatusCode::NOT_FOUND);
    }
    let username = body.username.trim();
    if username.is_empty() || body.password.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let (role, display_username) = if let Some(store) = state.users_store.as_ref() {
        match store.verify_login(username, &body.password).await {
            Ok(user) => (user.role, user.username),
            Err(nerve_waf_store::UsersError::Validation(_)) | Err(nerve_waf_store::UsersError::NotFound) => {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else if verify_env_login(username, &body.password) {
        ("admin".into(), username.to_string())
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    state
        .auth
        .issue_local_token(&display_username, &role)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
