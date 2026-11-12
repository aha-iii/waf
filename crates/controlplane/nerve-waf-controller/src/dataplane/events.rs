use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use nerve_waf_sdk::WafEventRecord;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct EdgeEventsBody {
    events: Vec<WafEventRecord>,
}

/// Shape B: after Edge block, report L7 events (PostgreSQL; visible in Console attack log),
/// and dispatch Nerve-alert for qualifying events (aligned with `/api/inspect`).
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
    let n = state.events.insert_batch(&events).await.map_err(|e| {
        tracing::error!("edge event ingest: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for ev in edge_alert_candidates(&events) {
        let triage = crate::triage_service::triage_one(state.as_ref(), ev).await;
        crate::events::spawn_alert_dispatch(state.as_ref(), ev, Some(triage));
    }

    Ok(Json(json!({ "ok": true, "inserted": n })))
}

/// Events that need alert dispatch after Edge persist (aligned with inspect `should_alert_waf_event`).
pub(crate) fn edge_alert_candidates(events: &[WafEventRecord]) -> Vec<&WafEventRecord> {
    events
        .iter()
        .filter(|e| crate::events::should_alert_waf_event(e))
        .collect()
}
