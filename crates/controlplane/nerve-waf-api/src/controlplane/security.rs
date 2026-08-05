//! 安全扩展路由：CRS、API 安全、防篡改、RBAC、License、Challenge

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_api_sec::{validate_request, ApiSecConfig};
use nerve_waf_challenge::ChallengeStore;
use nerve_waf_libinjection::LibInjectionConfig;
use nerve_waf_whitelist::WhitelistConfig;
use crate::platform::AutoDenyConfig;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn security_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/rules/packs", get(list_rules_packs).post(reload_rules_packs))
        .route("/api/rules/crs/versions", get(list_crs_versions))
        .route("/api/rules/crs/update", post(update_crs_rules))
        .route("/api/rules/crs/rollback", post(rollback_crs_rules))
        .route("/api/rules/crs/snapshot", post(snapshot_crs_rules))
        .route("/api/crs/runtime", get(get_crs_runtime).put(put_crs_runtime))
        .route("/api/libinjection/config", get(get_libinjection).put(put_libinjection))
        .route("/api/api-sec/config", get(get_api_sec).put(put_api_sec))
        .route("/api/api-sec/validate", post(api_sec_validate))
        .route("/api/tamper/config", get(get_tamper).put(put_tamper))
        .route("/api/tamper/check", post(tamper_check))
        .route("/api/rbac/config", get(get_rbac).put(put_rbac))
        .route("/api/license/usage", get(license_usage))
        .route("/api/challenge/issue", post(challenge_issue))
        .route("/api/challenge/verify", post(challenge_verify))
        .route("/api/challenge/page", get(challenge_page))
        .route("/api/whitelist", get(get_whitelist).put(put_whitelist))
        .route("/api/auto-deny", get(get_auto_deny).put(put_auto_deny))
}

async fn list_rules_packs(
    State(state): State<Arc<AppState>>,
    Query(q): Query<RulesPacksQuery>,
) -> Json<Value> {
    let inv = state.platform.rules_inventory(q.limit);
    Json(json!({
        "rules_dir": inv.rules_dir,
        "embedded_rules": inv.embedded_rules,
        "total_rules": inv.total_rules,
        "packs": inv.packs,
        "patches": inv.patches,
        "ai_packs": inv.ai_packs,
        "rules": inv.rules,
        "rules_total": inv.rules_total,
        "rules_truncated": inv.rules_truncated,
        "upgrade": {
            "import_script": "Nerve-waf/scripts/import-crs.sh",
            "reload": "POST /api/rules/packs",
            "libinjection": "GET/PUT /api/libinjection/config",
            "notes": "CRS @rx via nerve-waf-crs-import; libinjection 942100/941100 built-in"
        }
    }))
}

#[derive(Deserialize)]
struct RulesPacksQuery {
    #[serde(default = "default_rule_limit")]
    limit: usize,
}

fn default_rule_limit() -> usize {
    500
}

async fn reload_rules_packs(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let platform = state.platform.clone();
    let n = tokio::task::spawn_blocking(move || platform.reload_l7_rules())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.refresh_l7_from_store().await;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "rules": n })))
}

async fn list_crs_versions(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let versions = state
        .platform
        .crs_version_list()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "versions": versions,
        "max_versions": nerve_waf_rules_ops::MAX_CRS_VERSIONS,
        "rules_dir": state.platform.rules_dir_path(),
    })))
}

#[derive(Deserialize)]
struct CrsUpdateBody {
    #[serde(default = "default_crs_version")]
    crs_version: String,
    #[serde(default = "default_paranoia")]
    paranoia_max: u8,
}

fn default_crs_version() -> String {
    "v4.25.0".into()
}

fn default_paranoia() -> u8 {
    2
}

async fn update_crs_rules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CrsUpdateBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let platform = state.platform.clone();
    let crs_version = body.crs_version.clone();
    let paranoia_max = body.paranoia_max;

    let result = tokio::task::spawn_blocking(move || {
        platform.crs_update_latest(&crs_version, paranoia_max)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("CRS 更新任务异常: {e}") })),
        )
    })?;

    let (backup, stats, rules) = result.map_err(|e| {
        tracing::error!("crs update failed: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
    })?;
    state.refresh_l7_from_store().await;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "backup_version": backup,
        "import": stats,
        "rules": rules,
    })))
}

#[derive(Deserialize)]
struct CrsRollbackBody {
    version_id: String,
}

async fn rollback_crs_rules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CrsRollbackBody>,
) -> Result<Json<Value>, StatusCode> {
    let platform = state.platform.clone();
    let version_id = body.version_id.clone();
    let (version, rules) = tokio::task::spawn_blocking(move || platform.crs_rollback(&version_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|e| {
            tracing::error!("crs rollback failed: {e}");
            StatusCode::BAD_REQUEST
        })?;
    state.refresh_l7_from_store().await;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "restored": version,
        "rules": rules,
    })))
}

#[derive(Deserialize)]
struct CrsSnapshotBody {
    #[serde(default)]
    note: String,
}

async fn snapshot_crs_rules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CrsSnapshotBody>,
) -> Result<Json<Value>, StatusCode> {
    let note = if body.note.is_empty() {
        "手动快照".to_string()
    } else {
        body.note.clone()
    };
    let platform = state.platform.clone();
    let version = tokio::task::spawn_blocking(move || platform.crs_snapshot(&note))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "version": version })))
}

async fn get_libinjection(State(state): State<Arc<AppState>>) -> Json<Value> {
    let config = state.platform.libinjection_config();
    let status = state.platform.libinjection_status();
    Json(json!({
        "config": config,
        "status": status,
    }))
}

async fn put_libinjection(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<LibInjectionConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_libinjection_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "status": state.platform.libinjection_status(),
    })))
}

async fn get_api_sec(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.api_sec_config() }))
}

async fn put_api_sec(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<ApiSecConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_api_sec_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ApiSecValidateBody {
    host: String,
    method: String,
    path: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    body_len: usize,
    #[serde(default)]
    body: String,
}

async fn api_sec_validate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ApiSecValidateBody>,
) -> Json<Value> {
    let cfg = state.platform.api_sec_config();
    let v = validate_request(
        &cfg,
        &body.host,
        &body.method,
        &body.path,
        &body.query,
        body.body_len,
        &body.body,
    );
    Json(json!({ "violation": v }))
}

async fn get_tamper(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.tamper_config() }))
}

async fn put_tamper(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<nerve_waf_tamper::TamperConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_tamper_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TamperCheckBody {
    host: String,
    path: String,
    body: String,
}

async fn tamper_check(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TamperCheckBody>,
) -> Json<Value> {
    let alert = state
        .platform
        .tamper_guard()
        .check(&body.host, &body.path, body.body.as_bytes());
    Json(json!({ "alert": alert }))
}

async fn get_rbac(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.rbac_config() }))
}

async fn put_rbac(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<nerve_waf_rbac::RbacConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_rbac_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

async fn license_usage(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "usage": state.platform.usage_snapshot() }))
}

#[derive(Deserialize)]
struct ChallengeIssueBody {
    client_key: String,
}

async fn challenge_issue(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChallengeIssueBody>,
) -> Json<Value> {
    let token = state.platform.challenge_store().issue(&body.client_key);
    Json(json!({ "token": token, "ttl_sec": 300 }))
}

#[derive(Deserialize)]
struct ChallengeVerifyBody {
    token: String,
}

async fn challenge_verify(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChallengeVerifyBody>,
) -> Json<Value> {
    let ok = state.platform.challenge_store().verify(&body.token);
    Json(json!({ "ok": ok }))
}

#[derive(Deserialize)]
struct ChallengePageQuery {
    return_url: String,
    client_key: String,
    #[serde(default)]
    waf_api: String,
}

async fn challenge_page(Query(q): Query<ChallengePageQuery>) -> impl IntoResponse {
    let api = if q.waf_api.is_empty() {
        "http://127.0.0.1:8780".into()
    } else {
        q.waf_api
    };
    let html = ChallengeStore::challenge_html(&api, &q.return_url, &q.client_key);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charsett=utf-8")],
        html,
    )
}

async fn get_whitelist(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.whitelist_config() }))
}

async fn put_whitelist(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<WhitelistConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_whitelist_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn get_auto_deny(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.auto_deny_config() }))
}

async fn put_auto_deny(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<AutoDenyConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_auto_deny_config(cfg)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

async fn get_crs_runtime() -> Json<Value> {
    let status = nerve_waf_wasm::runtime_status();
    Json(serde_json::to_value(status).unwrap_or(json!({})))
}

#[derive(Deserialize)]
struct PutCrsRuntimeBody {
    engine: nerve_waf_wasm::CrsEnginePreference,
}

async fn put_crs_runtime(Json(body): Json<PutCrsRuntimeBody>) -> Result<Json<Value>, StatusCode> {
    let mut cfg = nerve_waf_wasm::load_runtime_config();
    cfg.engine = body.engine;
    nerve_waf_wasm::save_runtime_config(&cfg).map_err(|e| {
        tracing::error!("save crs runtime: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({
        "ok": true,
        "status": nerve_waf_wasm::runtime_status(),
    })))
}
