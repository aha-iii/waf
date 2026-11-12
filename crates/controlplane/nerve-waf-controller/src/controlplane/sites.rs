use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_cc::CcCheckRequest;
use nerve_waf_sdk::WafEventRecord;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::controlplane::alert_integration::{product::WAF, DispatchRequest};
use crate::events::{payload_from_event, should_alert_network_event};
use crate::platform::{validate_sites, ProtectedSite};
use crate::AppState;

#[derive(Deserialize)]
pub struct EventsBatchBody {
    #[serde(default)]
    pub events: Vec<WafEventRecord>,
}

pub fn platform_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/cc/config", get(get_cc_config).put(put_cc_config))
        .route("/api/cc/check", post(cc_check))
        .route("/api/geo/config", get(get_geo_config).put(put_geo_config))
        .route("/api/geo/lookup", post(geo_lookup))
        .route("/api/geo/reload", post(geo_reload))
        .route("/api/geo/status", get(geo_status))
        .route("/api/sites", get(get_sites).put(put_sites))
        .route("/api/gateway/sites", get(get_sites).put(put_sites))
        .route("/api/events/ingest", post(events_ingest))
        .route("/api/export/cef", post(export_cef))
}

async fn get_cc_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "config": state.platform.cc.config() }))
}

async fn put_cc_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<nerve_waf_cc::CcConfig>,
) -> Result<Json<Value>, StatusCode> {
    state.platform.cc.update_config(cfg);
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn cc_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CcCheckRequest>,
) -> Json<Value> {
    let result = state.platform.cc.check(&req);
    Json(serde_json::to_value(result).unwrap_or(json!({})))
}

async fn get_geo_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let geo = state.platform.geo.read().unwrap();
    Json(json!({
        "config": geo.config(),
        "status": geo.status(),
    }))
}

async fn put_geo_config(
    State(state): State<Arc<AppState>>,
    Json(cfg): Json<nerve_waf_geo::GeoConfig>,
) -> Result<Json<Value>, StatusCode> {
    {
        let mut geo = state.platform.geo.write().unwrap();
        geo.update_config(cfg);
    }
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct GeoLookupBody {
    ip: String,
}

async fn geo_lookup(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GeoLookupBody>,
) -> Json<Value> {
    let geo = state.platform.geo.read().unwrap();
    Json(serde_json::to_value(geo.lookup(&body.ip)).unwrap_or(json!({})))
}

async fn geo_reload(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let cidrs = state
        .platform
        .reload_geo_cidrs()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mmdb = state
        .platform
        .reload_geo_mmdb()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let status = state.platform.geo_status();
    Ok(Json(json!({
        "ok": true,
        "cidr_rules": cidrs,
        "mmdb_loaded": mmdb,
        "status": status,
    })))
}

async fn geo_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "status": state.platform.geo_status() }))
}

async fn get_sites(State(state): State<Arc<AppState>>) -> Json<Value> {
    let sites = state.platform.sites.read().unwrap().clone();
    Json(json!({ "sites": sites }))
}

async fn put_sites(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Vec<ProtectedSite>>,
) -> Result<Json<Value>, StatusCode> {
    validate_sites(&body).map_err(|e| {
        tracing::warn!("validate sites: {e}");
        StatusCode::BAD_REQUEST
    })?;
    let old_ids: std::collections::HashSet<String> = state
        .platform
        .sites
        .read()
        .unwrap()
        .iter()
        .map(|s| s.id.clone())
        .collect();
    let to_provision: Vec<String> = body
        .iter()
        .filter(|s| s.id != "default" && !old_ids.contains(&s.id))
        .map(|s| s.id.clone())
        .collect();
    {
        let mut sites = state.platform.sites.write().unwrap();
        *sites = body.clone();
    }
    if let Some(store) = &state.protected_sites_store {
        let values: Vec<Value> = body
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::warn!("serialize protected sites: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        store.replace_all(&values, "console").await.map_err(|e| {
            tracing::warn!("persist protected sites: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }
    for site_id in to_provision {
        state
            .config
            .ensure_app_provisioned(&site_id)
            .await
            .map_err(|e| {
                tracing::warn!("provision app {site_id}: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let _ = state.ai_settings.refresh_site_from_store(&site_id).await;
    }
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

async fn events_ingest(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EventsBatchBody>,
) -> Result<Json<Value>, StatusCode> {
    let events = normalize_events(body.events);
    let n = state
        .events
        .insert_batch(&events)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for ev in &events {
        if ev.layer == "l3" {
            if should_alert_network_event(ev) {
                state.alert_client.spawn_dispatch(DispatchRequest {
                    product: WAF.into(),
                    tenant: "default".into(),
                    trigger: "network".into(),
                    event_id: Some(ev.id.clone()),
                    test: false,
                    event: payload_from_event(ev),
                });
            }
        } else if ev.action == "block" || ev.severity == "high" || ev.severity == "critical" {
            state.alert_client.spawn_dispatch(DispatchRequest {
                product: WAF.into(),
                tenant: "default".into(),
                trigger: "ingest".into(),
                event_id: Some(ev.id.clone()),
                test: false,
                event: payload_from_event(ev),
            });
        }
    }

    Ok(Json(json!({ "ok": true, "inserted": n })))
}

fn normalize_events(mut events: Vec<WafEventRecord>) -> Vec<WafEventRecord> {
    use uuid::Uuid;
    for e in &mut events {
        if e.id.is_empty() {
            e.id = Uuid::new_v4().to_string();
        }
        if e.site_id.is_empty() {
            e.site_id = "default".into();
        }
    }
    events
}

#[derive(Deserialize)]
struct ExportBody {
    limit: Option<u32>,
}

async fn export_cef(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ExportBody>,
) -> Result<Json<Value>, StatusCode> {
    let limit = body.limit.unwrap_or(20).min(200);
    let result = state
        .events
        .query(nerve_waf_sdk::EventQuery {
            limit,
            offset: 0,
            ..Default::default()
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let lines: Vec<String> = result.events.iter().map(nerve_waf_export::to_cef).collect();
    Ok(Json(json!({ "lines": lines, "count": lines.len() })))
}
