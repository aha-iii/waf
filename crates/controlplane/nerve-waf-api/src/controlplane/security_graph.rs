//! 安全图谱 — WAF 事件与 Flow 关联

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::triage_routes::resolve_event;
use crate::AppState;

pub fn security_graph_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/security-graph/event/{event_id}", get(graph_by_event))
        .route("/api/security-graph/ip/{src_ip}", get(graph_by_ip))
        .route("/api/security-graph/block-ip", post(block_ip))
}

#[derive(Debug, Deserialize)]
struct EventGraphQuery {
    #[serde(default = "default_window")]
    window_minutes: i64,
}

fn default_window() -> i64 {
    30
}

#[derive(Debug, Deserialize)]
struct IpGraphQuery {
    #[serde(default = "default_window")]
    minutes: i64,
}

#[derive(Debug, Deserialize)]
struct BlockIpBody {
    ip: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    event_id: Option<String>,
}

async fn graph_by_event(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    Query(q): Query<EventGraphQuery>,
) -> Result<Json<Value>, StatusCode> {
    let event = resolve_event(&state, None, Some(event_id.clone())).await?;
    let src_ip = event.src_ip.clone().unwrap_or_default();
    let event_ts = event.ts;
    let window = q.window_minutes.clamp(1, 240);
    let from = event_ts - Duration::minutes(window);
    let to = event_ts + Duration::minutes(window / 2);

    let flows = fetch_flows(&state, &src_ip, from, to).await?;
    let graph = build_graph(&src_ip, &flows, Some(&event_id));
    Ok(Json(json!({
        "ok": true,
        "event": event,
        "flows": flows,
        "graph": graph,
        "highlight": graph_highlight(&src_ip, &flows),
        "window_minutes": window,
    })))
}

async fn graph_by_ip(
    State(state): State<Arc<AppState>>,
    Path(src_ip): Path<String>,
    Query(q): Query<IpGraphQuery>,
) -> Result<Json<Value>, StatusCode> {
    let to = Utc::now();
    let from = to - Duration::minutes(q.minutes.clamp(1, 1440));
    let flows = fetch_flows(&state, &src_ip, from, to).await?;
    let graph = build_graph(&src_ip, &flows, None);
    Ok(Json(json!({
        "ok": true,
        "src_ip": src_ip,
        "flows": flows,
        "graph": graph,
        "highlight": graph_highlight(&src_ip, &flows),
        "minutes": q.minutes,
    })))
}

async fn block_ip(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BlockIpBody>,
) -> Result<Json<Value>, StatusCode> {
    let ip = body.ip.trim();
    if ip.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let reason = body.reason.unwrap_or_else(|| {
        format!(
            "security-graph{}",
            body.event_id
                .as_deref()
                .map(|id| format!(" event={id}"))
                .unwrap_or_default()
        )
    });
    state
        .config
        .add_runtime_deny(ip, "security-graph", Some(&reason), Some("security-graph"), None)
        .await
        .map_err(|e| {
            tracing::error!("block-ip runtime deny: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state.nerve.deny_ip(ip).await.map_err(|e| {
        tracing::error!("block-ip nerve deny: {e}");
        StatusCode::BAD_GATEWAY
    })?;
    Ok(Json(json!({
        "ok": true,
        "ip": ip,
        "status": "denied",
        "kind": "temporary",
        "source": "security-graph",
    })))
}

async fn fetch_flows(
    state: &AppState,
    src_ip: &str,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
) -> Result<Vec<Value>, StatusCode> {
    if src_ip.is_empty() {
        return Ok(vec![]);
    }
    if let Some(topo) = &state.topology {
        if let Ok(rows) = topo.recent_flows(200, None).await {
            let filtered: Vec<Value> = rows
                .into_iter()
                .filter(|f| f.get("src_ip").and_then(|v| v.as_str()) == Some(src_ip))
                .collect();
            if !filtered.is_empty() {
                return Ok(filtered);
            }
        }
    }
    if let Some(pool) = state.events.postgres_pool() {
        return nerve_waf_store::flows_for_src_ip(&pool, src_ip, from, to, 200)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(vec![])
}

fn build_graph(src_ip: &str, flows: &[Value], event_id: Option<&str>) -> Value {
    let mut nodes: HashMap<String, Value> = HashMap::new();
    let mut edges: Vec<Value> = Vec::new();
    let ext_id = format!("ext:{src_ip}");
    nodes.insert(
        ext_id.clone(),
        json!({ "id": ext_id, "label": src_ip, "kind": "external" }),
    );
    if let Some(eid) = event_id {
        nodes.insert(
            format!("event:{eid}"),
            json!({ "id": format!("event:{eid}"), "label": "WAF Event", "kind": "event" }),
        );
        edges.push(json!({
            "source": ext_id,
            "target": format!("event:{eid}"),
            "directioe": "inboued",
            "count": 1,
        }));
    }
    for f in flows {
        let dst = f.get("dst_ip").and_then(|v| v.as_str()).unwrap_or("?");
        let pod = f.get("pod").and_then(|v| v.as_str());
        let workload = f.get("workload").and_then(|v| v.as_str());
        let target_label = pod.or(workload).unwrap_or(dst);
        let tid = format!("workload:{target_label}");
        nodes.entry(tid.clone()).or_insert(json!({
            "id": tid,
            "label": target_label,
            "kind": if pod.is_some() { "pod" } else { "host" },
        }));
        edges.push(json!({
            "source": ext_id,
            "target": tid,
            "directioe": f.get("directioe").cloned().unwrap_or(json!("unknown")),
            "count": 1,
        }));
    }
    json!({
        "nodes": nodes.values().collect::<Vec<_>>(),
        "edges": edges,
        "stats": {
            "flow_count": flows.len(),
            "node_count": nodes.len(),
        },
    })
}

fn graph_highlight(src_ip: &str, flows: &[Value]) -> Value {
    let mut nodes: HashSet<String> = HashSet::new();
    nodes.insert(format!("ext:{src_ip}"));
    for f in flows {
        if let Some(pod) = f.get("pod").and_then(|v| v.as_str()) {
            nodes.insert(format!("workload:{pod}"));
        }
    }
    json!({
        "nodes": nodes.iter().collect::<Vec<_>>(),
        "edges": [],
    })
}
