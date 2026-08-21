//! Network observation: trend stats, GeoIP lookup, Agent baseline-allow proxy.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
struct NetworkStatsQuery {
    #[serde(default = "default_days")]
    days: i64,
}

fn default_days() -> i64 {
    7
}

#[derive(Deserialize)]
struct GeoIpQuery {
    ip: String,
}

#[derive(Deserialize)]
struct BaselineAllowQuery {
    pod: String,
    ip: String,
}

#[derive(Deserialize)]
struct BaselineEnforceQuery {
    pod: Option<String>,
}

pub fn network_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/network/stats", get(network_stats))
        .route("/api/network/geoip", get(network_geoip))
        .route("/api/nerve/baseline/allow", post(nerve_baseline_allow))
        .route("/api/nerve/baseline/enforce", post(nerve_baseline_enforce))
}

async fn network_stats(
    State(state): State<Arc<AppState>>,
    Query(q): Query<NetworkStatsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let days = q.days.clamp(1, 30);
    let since = Utc::now() - Duration::days(days);
    let stats = state
        .events
        .network_stats_since(since, days)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "days": days,
        "since": since.to_rfc3339(),
        "backend": state.events.backend_name(),
        "stats": stats,
    })))
}

async fn network_geoip(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GeoIpQuery>,
) -> Json<Value> {
    let ip = q.ip.trim();
    let geo = state.platform.geo.read().unwrap().lookup(ip);
    Json(json!({
        "ip": geo.ip,
        "country": geo.country,
        "country_label": country_label(geo.country.as_deref()),
        "source": geo.source,
        "asn": Value::Null,
        "domain": Value::Null,
    }))
}

async fn nerve_baseline_allow(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BaselineAllowQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pod = q.pod.trim();
    let ip = q.ip.trim();
    if pod.is_empty() || ip.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "pod and ip required" })),
        ));
    }
    state
        .nerve
        .baseline_allow(pod, ip)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string() })),
            )
        })?;
    Ok(Json(json!({ "status": "ok", "pod": pod, "ip": ip })))
}

async fn nerve_baseline_enforce(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BaselineEnforceQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let pod = q.pod.as_deref().map(str::trim).filter(|s| !s.is_empty());
    state
        .nerve
        .baseline_enforce(pod)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string() })),
            )
        })?;
    Ok(Json(json!({ "status": "ok", "pod": pod })))
}

fn country_label(code: Option<&str>) -> String {
    match code.unwrap_or("").to_uppercase().as_str() {
        "CN" => "中国".into(),
        "US" => "美国".into(),
        "JP" => "日本".into(),
        "HK" => "中国香港".into(),
        "TW" => "中国台湾".into(),
        "PRIVATE" => "内网/保留".into(),
        "XX" | "" => "未知".into(),
        other => other.to_string(),
    }
}
