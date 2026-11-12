use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use nerve_waf_sdk::WafEventRecord;

use serde::Deserialize;

use serde_json::{json, Value};

use std::sync::Arc;

use crate::triage_ai::AiExplainResult;

use crate::triage_service::{triage_one, triage_to_json};

use crate::AppState;

pub fn triage_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/triage/analyze",
            post(analyze_triage).get(analyze_triage_get),
        )
        .route("/api/triage/explain", post(explain_triage))
        .route("/api/triage/config", get(triage_config))
}

#[derive(Deserialize)]

struct TriageBody {
    #[serde(default)]
    event: Option<WafEventRecord>,

    #[serde(default)]
    event_id: Option<String>,

    #[serde(default)]
    refresh: bool,
}

#[derive(Deserialize)]

struct TriageQuery {
    event_id: String,
}

pub(crate) async fn resolve_event(
    state: &AppState,

    event: Option<WafEventRecord>,

    event_id: Option<String>,
) -> Result<WafEventRecord, StatusCode> {
    if let Some(ev) = event {
        return Ok(ev);
    }

    let id = event_id.ok_or(StatusCode::BAD_REQUEST)?;

    let result = state
        .events
        .query(nerve_waf_sdk::EventQuery {
            limit: 500,

            ..Default::default()
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    result
        .events
        .into_iter()
        .find(|e| e.id == id)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn analyze_triage(
    State(state): State<Arc<AppState>>,

    Json(body): Json<TriageBody>,
) -> Result<Json<Value>, StatusCode> {
    let event = resolve_event(&state, body.event, body.event_id).await?;

    let triage = triage_one(&state, &event).await;

    let mut triage_json = triage_to_json(&triage);

    if let Some(ai) = state.triage_ai.get_cached(&event.id) {
        crate::triage_ai::TriageAiService::merge_ai_into_json(&mut triage_json, &ai);
    }

    Ok(Json(json!({ "event": event, "triage": triage_json })))
}

async fn analyze_triage_get(
    State(state): State<Arc<AppState>>,

    Query(q): Query<TriageQuery>,
) -> Result<Json<Value>, StatusCode> {
    let event = resolve_event(&state, None, Some(q.event_id)).await?;

    let triage = triage_one(&state, &event).await;

    let mut triage_json = triage_to_json(&triage);

    if let Some(ai) = state.triage_ai.get_cached(&event.id) {
        crate::triage_ai::TriageAiService::merge_ai_into_json(&mut triage_json, &ai);
    }

    Ok(Json(json!({ "event": event, "triage": triage_json })))
}

async fn explain_triage(
    State(state): State<Arc<AppState>>,

    Json(body): Json<TriageBody>,
) -> Result<Json<Value>, StatusCode> {
    let event = resolve_event(&state, body.event, body.event_id).await?;

    let triage = triage_one(&state, &event).await;

    let ai: AiExplainResult = state.triage_ai.explain(&event, &triage, body.refresh).await;

    let mut triage_json = triage_to_json(&triage);

    crate::triage_ai::TriageAiService::merge_ai_into_json(&mut triage_json, &ai);

    Ok(Json(json!({

        "event": event,

        "triage": triage_json,

        "ai": ai,

    })))
}

async fn triage_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let llm = state.ai_settings.triage_llm_config();
    Json(json!({
        "triage_engine": "multi_signal_fusion_v1",
        "llm_enabled": llm.enabled,
        "llm_ready": llm.llm_ready(),
        "llm_auto_explain": llm.auto_explain,
        "llm_model": llm.model,
        "llm_api_url": llm.api_url,
        "llm_api_key_set": !llm.api_key.is_empty(),
        "semantic_enabled": state.ai_settings.semantic_config().enabled,
        "settings_backend": state.ai_settings.backend_name(),
        "ai_explain_endpoint": "/api/triage/explain",
        "mcp_tools": ["waf_triage_analyze", "waf_triage_explain"],
    }))
}
