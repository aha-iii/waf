use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_store::{AgentRegisterRequest, FlowIngestBatch, WorkloadSyncBatch};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
struct GraphQuery {
    minutes: Option<i64>,
    direction: Option<String>,
    cluster: Option<String>,
}

#[derive(Deserialize)]
struct FlowsQuery {
    limit: Option<i64>,
    direction: Option<String>,
}

#[derive(Deserialize)]
struct WorkloadsQuery {
    cluster: Option<String>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct DockerInventoryQuery {
    node_id: Option<String>,
}

pub fn topology_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/topology/agents/register", post(register_agent))
        .route("/api/topology/agents", get(list_agents))
        .route("/api/topology/flows/ingest", post(ingest_flows))
        .route("/api/topology/flows", get(list_flows))
        .route("/api/topology/graph", get(topology_graph))
        .route("/api/topology/workloads/sync", post(sync_workloads))
        .route("/api/topology/workloads", get(list_workloads))
        .route("/api/topology/docker/sync", post(sync_docker_inventory))
        .route("/api/topology/docker/inventory", get(list_docker_inventory))
}

async fn register_agent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AgentRegisterRequest>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    topo.register_agent(body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

async fn ingest_flows(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FlowIngestBatch>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let n = topo
        .ingest_flows(body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "ingested": n })))
}

async fn sync_workloads(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WorkloadSyncBatch>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let n = topo
        .sync_workloads(body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "synced": n })))
}

async fn list_workloads(
    State(state): State<Arc<AppState>>,
    Query(q): Query<WorkloadsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let items = topo
        .list_workloads(q.cluster.as_deref(), q.limit.unwrap_or(100))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "workloads": items })))
}

async fn sync_docker_inventory(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let node_id = body.get("node_id").and_then(|v| v.as_str()).unwrap_or("");
    if node_id.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    topo.sync_docker_inventory(body)
        .await
        .map_err(|e| {
            tracing::error!("docker sync failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_docker_inventory(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DockerInventoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let inv = topo
        .docker_inventory_aggregate(q.node_id.as_deref(), 120)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(inv))
}

async fn list_agents(State(state): State<Arc<AppState>>) -> Json<Value> {
    let Some(topo) = state.topology.as_ref() else {
        return Json(json!({ "agents": [] }));
    };
    match topo.list_agents().await {
        Ok(agents) => Json(json!({ "agents": agents })),
        Err(e) => {
            tracing::error!("list_agents failed: {e}");
            Json(json!({ "agents": [] }))
        }
    }
}

async fn list_flows(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FlowsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let flows = topo
        .recent_flows(q.limit.unwrap_or(50), q.direction.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "flows": flows })))
}

async fn topology_graph(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GraphQuery>,
) -> Result<Json<Value>, StatusCode> {
    let topo = state.topology.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let graph = topo
        .graph_with_cluster(
            q.minutes.unwrap_or(15),
            q.direction.as_deref(),
            q.cluster.as_deref(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::to_value(graph).unwrap_or(json!({}))))
}
