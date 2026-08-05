//! 应用级 CRS 版本绑定（只读系统 rules-versions，不影响全局活跃规则集）

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_store::{crs_modules_snapshot, prune_disabled_rules, AppCrsStatus, CRS_MODULE_KEYS};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;

use crate::AppState;

pub fn app_crs_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/apps/{site_id}/crs", get(get_app_crs_status))
        .route("/api/apps/{site_id}/crs/history", get(list_app_crs_history))
        .route("/api/apps/{site_id}/crs/history/{history_id}", get(get_app_crs_history_entry))
        .route("/api/apps/{site_id}/crs/apply", post(apply_app_crs_version))
}

async fn get_app_crs_status(
    State(state): State<Arc<AppState>>,
    Path(site_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let binding = state
        .config
        .load_app_crs_binding(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let system_latest = state.platform.system_crs_latest_ref();
    let behind_system = match (&binding, &system_latest) {
        (Some(b), Some(sys)) => b.crs_version_id != sys.crs_version_id,
        (None, Some(_)) => true,
        _ => false,
    };
    let status = AppCrsStatus {
        binding,
        system_latest,
        behind_system,
    };
    Ok(Json(serde_json::to_value(status).unwrap_or(json!({}))))
}

async fn list_app_crs_history(
    State(state): State<Arc<AppState>>,
    Path(site_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let history = state
        .config
        .load_app_crs_history(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "site_id": site_id, "history": history })))
}

async fn get_app_crs_history_entry(
    State(state): State<Arc<AppState>>,
    Path((site_id, history_id)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    let entry = state
        .config
        .load_app_crs_history_entry(&site_id, &history_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(json!({ "entry": entry })))
}

#[derive(Deserialize)]
struct ApplyCrsBody {
    crs_version_id: String,
}

async fn apply_app_crs_version(
    State(state): State<Arc<AppState>>,
    Path(site_id): Path<String>,
    Json(body): Json<ApplyCrsBody>,
) -> Result<Json<Value>, StatusCode> {
    let version_id = body.crs_version_id.trim();
    if version_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let meta = state
        .platform
        .crs_version_meta(version_id)
        .ok_or(StatusCode::BAD_REQUEST)?;

    let version_dir = state.platform.crs_version_rules_dir(version_id);
    if !version_dir.is_dir() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let merged = state.platform.merged_l7_rules_from_version_dir(&version_dir);
    let valid_ids: HashSet<String> = merged.iter().map(|r| r.id.clone()).collect();

    state
        .config
        .ensure_app_provisioned(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut modules = state
        .config
        .load_modules_for(&site_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut snapshot = crs_modules_snapshot(&modules);
    prune_disabled_rules(&mut snapshot, &valid_ids);
    for key in CRS_MODULE_KEYS {
        if let Some(cfg) = snapshot.get(*key) {
            modules.modules.insert((*key).to_string(), cfg.clone());
        }
    }

    let binding = state
        .config
        .apply_app_crs_version(
            &site_id,
            version_id,
            meta.crs_version.as_deref(),
            meta.rule_count as u32,
            snapshot,
            "console",
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    state
        .config
        .save_modules_for(&site_id, &modules, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "site_id": site_id,
        "binding": binding,
        "message": "已应用 CRS 版本到本应用，系统全局规则集未变更",
    })))
}
