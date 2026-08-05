use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use nerve_waf_engine::sync_plan_with_l4;
use nerve_waf_np_compiler::{
    compile_all_k8s_np, compile_cnp, compile_east_west_l7, compile_fqdn_policy,
    compile_k8s_np, compile_k8s_np_full, compile_np_rules, CompiledL4Policy, DnsCacheSnapshot,
    EastWestL7Policy, FqdnPolicySpec, IdentityMap, K8sNetworkPolicySpec, NamespaceSnapshot,
    NerveCNPolicySpec, NpRule, PodSnapshot,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn np_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/np/compile", post(np_compile))
        .route("/api/np/compile-l4", post(np_compile_l4))
        .route("/api/np/compile-cnp", post(np_compile_cnp))
        .route("/api/np/compile-fqdn", post(np_compile_fqdn))
        .route("/api/np/sync", post(np_sync))
        .route("/api/np/sync-all", post(np_sync_all))
        .route("/api/np/east-west", post(np_east_west))
        .route("/api/service-policies", get(list_service_policies).put(put_service_policies))
}

#[derive(Deserialize)]
struct NpCompileBody {
    rules: Vec<NpRule>,
}

async fn np_compile(Json(body): Json<NpCompileBody>) -> Json<Value> {
    Json(json!({ "plan": compile_np_rules(&body.rules) }))
}

#[derive(Deserialize)]
struct NpK8sBody {
    spec: K8sNetworkPolicySpec,
    #[serde(default)]
    pods: Vec<PodSnapshot>,
    #[serde(default)]
    namespaces: Vec<NamespaceSnapshot>,
}

async fn np_compile_l4(Json(body): Json<NpK8sBody>) -> Json<Value> {
    let (plan, l4, id_map) = compile_k8s_np_full(&body.spec, &body.pods, &body.namespaces);
    Json(json!({ "plan": plan, "l4_policy": l4, "identity": id_map }))
}

#[derive(Deserialize)]
struct NpCnpBody {
    spec: NerveCNPolicySpec,
    #[serde(default)]
    pods: Vec<PodSnapshot>,
}

async fn np_compile_cnp(Json(body): Json<NpCnpBody>) -> Json<Value> {
    Json(json!({ "cnp": compile_cnp(&body.spec, &body.pods) }))
}

#[derive(Deserialize)]
struct NpFqdnBody {
    spec: FqdnPolicySpec,
    #[serde(default)]
    pods: Vec<PodSnapshot>,
    #[serde(default)]
    dns: DnsCacheSnapshot,
}

async fn np_compile_fqdn(Json(body): Json<NpFqdnBody>) -> Json<Value> {
    Json(json!({ "fqdn": compile_fqdn_policy(&body.spec, &body.pods, &body.dns) }))
}

async fn np_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NpK8sBody>,
) -> Result<Json<Value>, StatusCode> {
    let (plan, l4, id_map) = if body.pods.is_empty() {
        (compile_k8s_np(&body.spec), CompiledL4Policy::default(), IdentityMap::default())
    } else {
        compile_k8s_np_full(&body.spec, &body.pods, &body.namespaces)
    };
    let payload = sync_plan_with_l4(&plan, &l4, &id_map);
    sync_to_nerve_payload(&state, &payload).await.map(Json).map_err(|_| StatusCode::BAD_GATEWAY)
}

#[derive(Deserialize)]
struct NpSyncAllBody {
    specs: Vec<K8sNetworkPolicySpec>,
    pods: Vec<PodSnapshot>,
    #[serde(default)]
    namespaces: Vec<NamespaceSnapshot>,
}

async fn np_sync_all(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NpSyncAllBody>,
) -> Result<Json<Value>, StatusCode> {
    let (plan, l4, id_map) = compile_all_k8s_np(&body.specs, &body.pods, &body.namespaces);
    let payload = sync_plan_with_l4(&plan, &l4, &id_map);
    let result = sync_to_nerve_payload(&state, &payload).await?;
    Ok(Json(json!({
        "l4_rules": l4.rules.len(),
        "lpm_rules": l4.lpm_rules.len(),
        "endpoints": l4.endpoints.len(),
        "identities": id_map.records.len(),
        "result": result,
    })))
}

async fn np_east_west(Json(policies): Json<Vec<EastWestL7Policy>>) -> Json<Value> {
    Json(json!({ "plan": compile_east_west_l7(&policies) }))
}

async fn list_service_policies(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "policies": state.platform.service_policies() }))
}

async fn put_service_policies(
    State(state): State<Arc<AppState>>,
    Json(policies): Json<Vec<nerve_waf_service_policy::ServiceAllowPolicy>>,
) -> Result<Json<Value>, StatusCode> {
    state
        .platform
        .set_service_policies(policies)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

async fn sync_to_nerve_payload(state: &AppState, payload: &Value) -> Result<Value, StatusCode> {
    // Cluster NP / L4 / identity → source=control (not WAF-L3).
    state
        .nerve
        .sync_plan_control(payload)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)
}
