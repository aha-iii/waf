use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use nerve_waf_sdk::WafEventRecord;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct EdgeEventsBody {
    events: Vec<WafEventRecord>,
}

/// 形态 B：Edge 拦截后上报 L7 事件（写入 PostgreSQL，Console 攻击日志可见）。
pub async fn post_waf_edge_events(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EdgeEventsBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut events = body.events;
    for e in &mut events {
        if e.id.is_empty() {
            e.id = Uuid::new_v4().to_string();
        }
        if e.ts.timestamp() == 0 {
            e.ts = Utc::now();
        }
        if e.site_id.is_empty() {
            e.site_id = "default".into();
        }
        if e.layer.is_empty() {
            e.layer = "l7".into();
        }
        if e.inspect_source.is_none() {
            e.inspect_source = Some("waf-edge".into());
        }
    }
    let n = state
        .events
        .insert_batch(&events)
        .await
        .map_err(|e| {
            tracing::error!("edge event ingest: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "ok": true, "inserted": n })))
}
