//! Phase 10 — Cluster Mesh + DNS policy + Identity aggregate API.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn phase10_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/clusters/register", post(register_cluster))
        .route("/api/clusters", get(list_clusters))
        .route("/api/dns-policy/evaluate", get(dns_evaluate))
        .route("/api/identity/lookup", get(identity_lookup))
}

#[derive(Deserialize)]
struct ClusterRegister {
    cluster_id: String,
    relay_url: Option<String>,
    agent_count: Option<u64>,
}

async fn register_cluster(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClusterRegister>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .register_cluster(body.cluster_id.clone(), body.relay_url.clone(), body.agent_count.unwrap_or(0))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "cluster_id": body.cluster_id })))
}

async fn list_clusters(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "clusters": state.platform.list_clusters() }))
}

#[derive(Deserialize)]
struct DnsQuery {
    namespace: String,
    fqdn: String,
}

async fn dns_evaluate(Query(q): Query<DnsQuery>) -> Json<Value> {
    let base = std::env::var("NERVE_DNS_POLICY_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".into());
    let url = format!(
        "{}/api/dns-policy/evaluate?namespace={}&fqdn={}",
        base.trim_end_matches('/'),
        urlencoding::encode(&q.namespace),
        urlencoding::encode(&q.fqdn)
    );
    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => Json(r.json().await.unwrap_or(json!({}))),
        Ok(r) => Json(json!({ "allowed": false, "error": r.status().to_string() })),
        Err(e) => Json(json!({ "allowed": true, "fallback": true, "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct IdentityQuery {
    namespace: String,
    pod: String,
}

async fn identity_lookup(Query(q): Query<IdentityQuery>) -> Json<Value> {
    let base = std::env::var("NERVE_IDENTITY_URL").unwrap_or_else(|_| "http://127.0.0.1:8085".into());
    let url = format!(
        "{}/api/identity/lookup?namespace={}&pod={}",
        base.trim_end_matches('/'),
        urlencoding::encode(&q.namespace),
        urlencoding::encode(&q.pod)
    );
    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => Json(r.json().await.unwrap_or(json!({}))),
        _ => Json(json!({ "spiffe_id": format!("spiffe://nerve.local/ns/{}/pod/{}", q.namespace, q.pod) })),
    }
}
