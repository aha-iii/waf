use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

use nerve_waf_dataplane::WafEdgeSnapshot;
use nerve_waf_sdk::L7Upstream;

use crate::dataplane::generation::dataplane_generation;
use crate::platform::ProtectedSite;
use crate::AppState;

use super::build::compile_waf_edge_snapshot;

pub fn dataplane_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/waf/edge/dataplane", get(get_waf_edge_dataplane))
        .route("/api/waf/edge/watch", get(watch_waf_edge_dataplane))
        .route("/api/waf/edge/status", get(get_waf_edge_status))
        .route("/api/waf/edge/ack", post(post_waf_edge_ack))
        .route(
            "/api/waf/edge/events",
            post(super::events::post_waf_edge_events),
        )
}

#[derive(Debug, Deserialize)]
struct WatchQuery {
    since: u64,
    #[serde(default = "default_timeout")]
    timeout_sec: u64,
}

fn default_timeout() -> u64 {
    30
}

async fn get_waf_edge_dataplane(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let snap = compile_waf_edge_snapshot(&state).await?;
    Ok(Json(snapshot_to_json(&snap)))
}

async fn get_waf_edge_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "generation": dataplane_generation(&state),
        "last_edge_ack_generation": state.last_edge_ack_generation.load(std::sync::atomic::Ordering::SeqCst),
    }))
}

#[derive(Debug, Deserialize)]
struct EdgeAckBody {
    generation: u64,
}

async fn post_waf_edge_ack(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EdgeAckBody>,
) -> Json<Value> {
    state
        .last_edge_ack_generation
        .fetch_max(body.generation, std::sync::atomic::Ordering::SeqCst);
    Json(json!({
        "ok": true,
        "ack_generation": body.generation,
    }))
}

async fn watch_waf_edge_dataplane(
    State(state): State<Arc<AppState>>,
    Query(q): Query<WatchQuery>,
) -> Result<Json<Value>, StatusCode> {
    let timeout = q.timeout_sec.clamp(1, 120);
    let current = dataplane_generation(&state);
    if q.since >= current {
        let mut rx = state.dataplane_gen.subscribe();
        let wait = async {
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let gen = *rx.borrow();
                if gen > q.since {
                    break;
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(timeout), wait).await;
    }
    let snap = compile_waf_edge_snapshot(&state).await?;
    let gen = snap.generation;
    if gen <= q.since {
        return Ok(Json(json!({
            "ok": true,
            "generation": gen,
            "unchanged": true,
        })));
    }
    Ok(Json(snapshot_to_json(&snap)))
}

fn snapshot_to_json(snap: &WafEdgeSnapshot) -> Value {
    json!({
        "ok": true,
        "generation": snap.generation,
        "checksum": snap.checksum,
        "built_at": snap.built_at,
        "dataplane": snap,
    })
}

pub(crate) fn is_gateway_only_site(site: &ProtectedSite) -> bool {
    let m = site.ingress_mode.trim().to_lowercase();
    let ig = site.integration.trim().to_lowercase();
    m == "nerve-gateway" || m == "gateway-bypass" || ig == "nerve-gateway"
}

pub(crate) fn origins_to_l7_upstream(site: &ProtectedSite, upstream_id: &str) -> L7Upstream {
    let nodes: Vec<nerve_waf_sdk::L7UpstreamNode> = site
        .origins
        .iter()
        .filter(|o| !o.address.trim().is_empty())
        .map(|o| nerve_waf_sdk::L7UpstreamNode {
            host: o.address.trim().to_string(),
            port: o.port,
            weight: o.weight.max(1),
        })
        .collect();
    let scheme = site
        .origins
        .first()
        .map(|o| o.scheme.clone())
        .unwrap_or_else(|| "http".into());
    L7Upstream {
        id: upstream_id.to_string(),
        name: format!("{} 回源", site.name),
        enabled: site.enabled,
        origin_kind: Some("direct".into()),
        scheme,
        upstream_type: "roundrobin".into(),
        nodes,
        description: Some(format!("WAF edge inline origins for {}", site.id)),
        host_header: site.origins.first().and_then(|o| o.host_header.clone()),
        sni: site.origins.first().and_then(|o| o.sni.clone()),
        transport: None,
        checks: None,
        health_check: None,
    }
}

pub(crate) async fn collect_edge_routes(
    state: &AppState,
) -> Result<
    (
        Vec<nerve_waf_dataplane::EdgeRoute>,
        Vec<L7Upstream>,
        Vec<nerve_waf_dataplane::EdgeSiteMeta>,
        HashMap<String, nerve_waf_dataplane::SiteInspectSnapshot>,
    ),
    StatusCode,
> {
    use nerve_waf_dataplane::{EdgeRoute, EdgeSiteMeta, SiteInspectSnapshot};

    let sites = state
        .platform
        .sites
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .clone();
    let mut store_upstreams = state
        .config
        .load_upstreams()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    for u in &mut store_upstreams {
        u.normalize_checks();
    }
    let mut upstream_map: HashMap<String, L7Upstream> = store_upstreams
        .into_iter()
        .map(|u| (u.id.clone(), u))
        .collect();

    let mut routes = Vec::new();
    let mut edge_sites = Vec::new();
    let mut edge_upstreams = Vec::new();
    let mut site_inspect: HashMap<String, SiteInspectSnapshot> = HashMap::new();
    let mut seen_up = HashSet::new();

    for site in sites
        .iter()
        .filter(|s| s.enabled && !is_gateway_only_site(s))
    {
        let upstream_id = site
            .gateway_upstream_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("up-{}", site.id));

        if upstream_map.contains_key(&upstream_id) {
            // bound
        } else if !site.origins.is_empty() {
            upstream_map.insert(
                upstream_id.clone(),
                origins_to_l7_upstream(site, &upstream_id),
            );
        } else {
            continue;
        }

        let up = match upstream_map.get(&upstream_id) {
            Some(u) if !u.is_gateway_origin() && u.enabled => u,
            _ => continue,
        };

        if seen_up.insert(upstream_id.clone()) {
            edge_upstreams.push(up.clone());
        }

        edge_sites.push(EdgeSiteMeta {
            id: site.id.clone(),
            name: site.name.clone(),
            hosts: site.hosts.clone(),
            ingress_mode: site.ingress_mode.clone(),
            upstream_id: upstream_id.clone(),
        });

        if let Ok(inputs) = crate::site_inspect::load_site_inspect_inputs(state, &site.id).await {
            site_inspect.insert(site.id.clone(), inputs.snapshot);
        }

        for host in &site.hosts {
            let host = host.trim().to_lowercase();
            if host.is_empty() {
                continue;
            }
            routes.push(EdgeRoute {
                id: format!(
                    "waf-edge-{}-{}",
                    site.id,
                    host.replace('.', "-").replace('*', "wildcard")
                ),
                site_id: site.id.clone(),
                host,
                upstream_id: upstream_id.clone(),
            });
        }
    }

    Ok((routes, edge_upstreams, edge_sites, site_inspect))
}
