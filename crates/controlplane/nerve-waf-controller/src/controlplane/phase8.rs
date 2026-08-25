//! Phase 8 — CNI eBPF datapath + NP enforcement API.

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_engine::sync_to_nerve;
use nerve_waf_np_compiler::{compile_k8s_np, K8sNetworkPolicySpec};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn phase8_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/datapath/status", get(datapath_status))
        .route("/api/nerve/cni/endpoints", get(cni_endpoints))
        .route("/api/nerve/cni/network", get(cni_network))
        .route("/api/np/enforce", post(np_enforce))
}

/// Proxy Agent `GET /api/cni/endpoints` — Pod IP metadata for topology.
async fn cni_endpoints(State(state): State<Arc<AppState>>) -> Json<Value> {
    match state.nerve.get("/api/cni/endpoints").await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({
            "endpoints": [],
            "error": e.to_string(),
        })),
    }
}

/// Cluster networks / subnets (P2 CRD aggregate; empty when not ready; Console falls back to Agent cluster_cidrs).
async fn cni_network(State(state): State<Arc<AppState>>) -> Json<Value> {
    match state.nerve.get("/api/cni/network").await {
        Ok(v) => Json(v),
        Err(_) => Json(json!({
            "cluster_id": "default",
            "source": "empty",
            "network": {},
            "subnets": [],
        })),
    }
}

async fn datapath_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let agent = state.nerve.get("/api/datapath/status").await.ok();
    Json(json!({
        "phase": 8,
        "agent": agent.unwrap_or(json!({ "ok": false })),
        "compiler": "nerve-waf-np-compiler",
    }))
}

#[derive(Deserialize)]
struct NpEnforceBody {
    spec: K8sNetworkPolicySpec,
    #[serde(default)]
    enforce: bool,
}

async fn np_enforce(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NpEnforceBody>,
) -> Result<Json<Value>, StatusCode> {
    let plan = compile_k8s_np(&body.spec);
    if !body.enforce {
        return Ok(Json(json!({ "ok": true, "dry_run": true, "plan": plan })));
    }
    let result = sync_to_nerve(&state.nerve, &plan).await;
    if !result.ok {
        return Err(StatusCode::BAD_GATEWAY);
    }
    Ok(Json(json!({
        "ok": result.ok,
        "enforced": true,
        "applied": result.applied,
        "errors": result.errors
    })))
}
