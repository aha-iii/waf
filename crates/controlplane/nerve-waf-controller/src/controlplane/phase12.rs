//! Phase 12 — ClickHouse export + AI rules + Mesh status API.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn phase12_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/flows/export/ch", post(export_clickhouse))
        .route("/api/mesh/status", get(mesh_status))
        .route("/api/ai/rules", get(list_ai_rules))
}

#[derive(Deserialize)]
struct ChExportBody {
    #[serde(default = "default_minutes")]
    minutes: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_minutes() -> i64 {
    60
}
fn default_limit() -> i64 {
    1000
}

async fn export_clickhouse(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChExportBody>,
) -> Json<Value> {
    let flows = if let Some(topo) = &state.topology {
        topo.recent_flows(body.limit, None).await.unwrap_or_default()
    } else {
        vec![]
    };
    Json(json!({
        "backend": "clickhouse-stub",
        "format": "jsonl",
        "minutes": body.minutes,
        "count": flows.len(),
        "hint": "Set CLICKHOUSE_URL for native export; Phase 12 MVP returns PG flows",
        "flows": flows
    }))
}

async fn mesh_status() -> Json<Value> {
    let base = std::env::var("NERVE_MESH_URL").unwrap_or_else(|_| "http://127.0.0.1:8086".into());
    let client = reqwest::Client::new();
    match client.get(format!("{}/api/mesh/status", base.trim_end_matches('/'))).send().await {
        Ok(r) if r.status().is_success() => Json(r.json().await.unwrap_or(json!({}))),
        _ => Json(json!({ "mesh": "offline", "mtls": false, "phase": 12 })),
    }
}

async fn list_ai_rules(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "packs": state.platform.ai_rule_packs(),
        "total_rules": state.platform.l7_rule_count()
    }))
}
