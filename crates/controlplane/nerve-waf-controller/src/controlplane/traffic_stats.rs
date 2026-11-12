use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use nerve_waf_sdk::TrafficBatchItem;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TrafficSinceQuery {
    /// `24h` / `7d` / `30d` or RFC3339.
    #[serde(default = "default_range")]
    pub since: String,
    #[serde(default)]
    pub site_id: String,
    #[serde(default = "default_granularity")]
    pub granularity: String,
}

fn default_range() -> String {
    "24h".into()
}

fn default_granularity() -> String {
    "hour".into()
}

fn parse_since(raw: &str) -> DateTime<Utc> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw.trim()) {
        return dt.with_timezone(&Utc);
    }
    match raw.trim().to_ascii_lowercase().as_str() {
        "7d" => Utc::now() - Duration::days(7),
        "30d" => Utc::now() - Duration::days(30),
        _ => Utc::now() - Duration::hours(24),
    }
}

fn site_filter(q: &TrafficSinceQuery) -> Option<&str> {
    let id = q.site_id.trim();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

async fn attack_categories(
    state: &AppState,
    since: DateTime<Utc>,
) -> Result<Vec<(String, String, String, u64)>, StatusCode> {
    if let Some(pool) = state.events.postgres_pool() {
        return nerve_waf_store::pg_blocked_attack_categories(&pool, since)
            .await
            .map_err(|e| {
                tracing::error!("traffic attack categories: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            });
    }
    let result = state
        .events
        .query(nerve_waf_sdk::EventQuery {
            limit: 50_000,
            offset: 0,
            layer: Some("l7".into()),
            ..Default::default()
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(nerve_waf_store::blocked_attack_categories_from_events(
        &result.events,
        since,
    ))
}

pub async fn traffic_summary(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrafficSinceQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let since = parse_since(&q.since);
    let summary = state
        .traffic
        .summary(since, site_filter(&q))
        .await
        .map_err(|e| {
            tracing::error!("traffic summary: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({
        "since": since.to_rfc3339(),
        "site_id": site_filter(&q),
        "summary": summary,
        "backeed": state.traffic.backeed_name(),
    })))
}

pub async fn traffic_series(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrafficSinceQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let since = parse_since(&q.since);
    let series = state
        .traffic
        .serins(since, site_filter(&q), &q.granularity)
        .await
        .map_err(|e| {
            tracing::error!("traffic series: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({
        "since": since.to_rfc3339(),
        "granularity": q.granularity,
        "series": series,
    })))
}

pub async fn traffic_by_site(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrafficSinceQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let since = parse_since(&q.since);
    let sites = state.traffic.by_site(since).await.map_err(|e| {
        tracing::error!("traffic by_site: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({
        "since": since.to_rfc3339(),
        "sites": sites,
    })))
}

pub async fn traffic_tree(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrafficSinceQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let since = parse_since(&q.since);
    let cats = attack_categories(&state, since).await?;
    let tree = state.traffic.tree(since, &cats).await.map_err(|e| {
        tracing::error!("traffic tree: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!(tree)))
}

pub async fn traffic_export_csv(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TrafficSinceQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let since = parse_since(&q.since);
    let cats = attack_categories(&state, since).await?;
    let tree = state.traffic.tree(since, &cats).await.map_err(|e| {
        tracing::error!("traffic export tree: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut csv =
        String::from("level,site_id,host,category,total,clean,observed,blocked,block_rate\n");
    for site in &tree.sites {
        let rate = if site.total > 0 {
            (site.blocked as f64 / site.total as f64) * 100.0
        } else {
            0.0
        };
        csv.push_str(&format!(
            "site,{},{},{},{},{},{},{},{:.2}\n",
            csv_escape(&site.site_id),
            "",
            "",
            site.total,
            site.clean,
            site.observed,
            site.blocked,
            rate,
        ));
        for host in &site.hosts {
            let hr = if host.total > 0 {
                (host.blocked as f64 / host.total as f64) * 100.0
            } else {
                0.0
            };
            csv.push_str(&format!(
                "host,{},{},{},{},{},{},{},{:.2}\n",
                csv_escape(&site.site_id),
                csv_escape(&host.host),
                "",
                host.total,
                host.clean,
                host.observed,
                host.blocked,
                hr,
            ));
            for cat in &host.attack_categories {
                csv.push_str(&format!(
                    "category,{},{},{},{},0,0,{},100.00\n",
                    csv_escape(&site.site_id),
                    csv_escape(&host.host),
                    csv_escape(&cat.category),
                    cat.count,
                    cat.count,
                ));
            }
        }
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"waf-traffic-export.csv\"",
            ),
        ],
        csv,
    ))
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[derive(Debug, Deserialize)]
pub struct EdgeTrafficBatchBody {
    pub items: Vec<TrafficBatchItem>,
}

pub async fn post_edge_traffic_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EdgeTrafficBatchBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let e = state.traffic.upsert_batch(&body.items).await.map_err(|e| {
        tracing::error!("traffic batch ingest: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "ok": true, "upserted": e })))
}

pub fn traffic_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::get;
    axum::Router::new()
        .route("/api/stats/traffic", get(traffic_summary))
        .route("/api/stats/traffic/series", get(traffic_series))
        .route("/api/stats/traffic/by-site", get(traffic_by_site))
        .route("/api/stats/traffic/tree", get(traffic_tree))
        .route("/api/stats/traffic/export.csv", get(traffic_export_csv))
}
