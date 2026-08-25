use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use nerve_waf_engine::{compile_config_with_runtime, sync_to_nerve};
use nerve_waf_sdk::{SyncPlan, SyncResult, WafConfig, WafRoute};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;

use crate::events;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/plugins", get(plugins))
        .route("/api/plugins/reload", post(reload_plugins))
        .route("/api/expert/reload", post(expert_reload))
        .route("/api/expert/health", get(expert_health))
        .route("/api/routes", get(get_routes).put(put_routes).post(post_route))
        .route("/api/routes/{id}", delete(delete_route))
        .route("/api/modules", get(events::get_modules).put(events::put_modules))
        .route(
            "/api/global/modules",
            get(events::get_global_modules).put(events::put_global_modules),
        )
        .route("/api/global/modules/publish", post(events::publish_global_modules))
        .route("/api/events", get(events::get_events))
        .route("/api/events/batch", post(events::post_events_batch))
        .route("/api/events/poll", post(events::poll_nerve_events))
        .route("/api/stats/overview", get(events::stats_overview))
        .route("/api/internal/stats", get(events::internal_stats))
        .route("/api/inspect", post(events::post_inspect))
        .route("/api/db/status", get(db_status))
        .route("/api/compile", post(compile))
        .route("/api/sync", post(sync))
        .route("/api/nerve/status", get(nerve_status))
        .route("/api/nerve/observer/live", get(nerve_observer_live))
        .route("/api/nerve/observer/sessions", get(nerve_observer_sessions))
        .route("/api/nerve/observer/session", get(nerve_observer_session))
        .route("/api/nerve/observer/baseline", get(nerve_observer_baseline))
        .route("/api/nerve/observer/anomalies", get(nerve_observer_anomalies))
        .route("/api/nerve/observer/policy-hits", get(nerve_observer_policy_hits))
        .route("/api/nerve/observer/explain", get(nerve_observer_explain))
        .route("/api/nerve/observer/journeys", get(nerve_observer_journeys))
        .route("/api/nerve/observer/journey", get(nerve_observer_journey))
        .route("/api/nerve/observer/trace/genealogy", get(nerve_observer_trace_genealogy))
        .route("/api/policy/deny", post(policy_deny))
        .route("/api/policy/allow", post(policy_allow))
        .route("/api/policy/blacklist", get(policy_blacklist))
        .route("/api/runtime-denies", get(list_runtime_denies))
        .route("/api/topology/status", get(topology_status))
}

async fn topology_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "enabled": state.topology.is_some(),
        "databaseConfigured": state
            .database_url
            .as_ref()
            .is_some_and(|u| !u.trim().is_empty()),
    }))
}

/// Anonymous health probe: liveness only; no DB/path/upstream details.
/// Detailed diagnosis via authenticated `/api/db/status`.
async fn health(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn db_status(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let status = state
        .events
        .db_status()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "ok": true,
        "configBackend": state.config.backend_name(),
        "db": status,
    })))
}

async fn plugins(State(state): State<Arc<AppState>>) -> Json<Value> {
    let host = state.plugin_host.read().unwrap();
    let plugins = host.list_descriptors();
    Json(json!({
        "plugins": plugins,
        "pluginsDir": host.plugins_dir().display().to_string(),
        "ruleCount": state.l7_rule_count(),
        "errors": host.last_errors(),
    }))
}

async fn reload_plugins(State(state): State<Arc<AppState>>) -> Json<Value> {
    let report = state.reload_plugins_and_l7_async().await;
    Json(json!({
        "ok": report.ok,
        "report": report,
        "ruleCount": state.l7_rule_count(),
    }))
}

/// Hot-reload Nerve-waf-expert pack (`NERVE_EXPERT_PACK` or default candidates).
async fn expert_reload() -> Result<Json<Value>, StatusCode> {
    match nerve_waf_inspect::reload_engine(None) {
        Ok(()) => {
            let health = nerve_waf_inspect::expert_health();
            Ok(Json(json!({
                "ok": true,
                "reloaded": true,
                "health": health,
            })))
        }
        Err(e) => Ok(Json(json!({
            "ok": false,
            "error": e,
        }))),
    }
}

async fn expert_health() -> Json<Value> {
    #[cfg(not(feature = "expert"))]
    {
        return Json(json!({
            "ok": false,
            "error": "expert engine not compiled (build with --features expert)",
            "last_load_error": nerve_waf_inspect::expert_last_load_error(),
            "enabled": nerve_waf_inspect::expert_engine_enabled(),
        }));
    }

    #[cfg(feature = "expert")]
    match nerve_waf_inspect::expert_health() {
        Some(h) => {
            let metrics = nerve_waf_inspect::expert_metrics_snapshot().map(|s| {
                json!({
                    "inspect_total": s.inspect_total,
                    "inspect_fast": s.inspect_fast,
                    "inspect_deep": s.inspect_deep,
                    "fast_pool_escalate": s.fast_pool_escalate,
                    "route_gate_deep": s.route_gate_deep,
                    "latency_fast_us_avg": avg_us(s.latency_fast_us_sum, s.inspect_fast),
                    "latency_deep_us_avg": avg_us(s.latency_deep_us_sum, s.inspect_deep),
                })
            });
            Json(json!({ "ok": true, "health": h, "metrics": metrics }))
        }
        None => Json(json!({
            "ok": false,
            "error": "expert engine not loaded",
            "last_load_error": nerve_waf_inspect::expert_last_load_error(),
            "enabled": nerve_waf_inspect::expert_engine_enabled(),
        })),
    }
}

#[cfg(feature = "expert")]
fn avg_us(sum: u64, n: u64) -> u64 {
    if n == 0 { 0 } else { sum / n }
}

async fn get_routes(State(state): State<Arc<AppState>>) -> Result<Json<WafConfig>, StatusCode> {
    state
        .config
        .load_config()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn put_routes(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WafConfig>,
) -> Result<Json<Value>, StatusCode> {
    state
        .config
        .save_config(&body, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true })))
}

async fn post_route(
    State(state): State<Arc<AppState>>,
    Json(route): Json<WafRoute>,
) -> Result<Json<Value>, StatusCode> {
    if route.id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut config = state
        .config
        .load_config()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(idx) = config.routes.iter().position(|r| r.id == route.id) {
        config.routes[idx] = route.clone();
    } else {
        config.routes.push(route.clone());
    }

    state
        .config
        .save_config(&config, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "ok": true, "route": route })))
}

async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut config = state
        .config
        .load_config()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let before = config.routes.len();
    config.routes.retain(|r| r.id != id);
    if config.routes.len() == before {
        return Err(StatusCode::NOT_FOUND);
    }

    state
        .config
        .save_config(&config, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "ok": true })))
}

async fn build_sync_plan(state: &AppState) -> Result<SyncPlan, StatusCode> {
    state.reload_plugins_and_l7_async().await;
    let config = state
        .config
        .load_config()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let modules = state
        .config
        .load_modules()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let runtime = state
        .config
        .active_runtime_deny_ips()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(compile_config_with_runtime(
        &config,
        &modules,
        state.l7_rule_count(),
        &runtime,
    ))
}

async fn compile(State(state): State<Arc<AppState>>) -> Result<Json<SyncPlan>, StatusCode> {
    Ok(Json(build_sync_plan(&state).await?))
}

async fn sync(
    State(state): State<Arc<AppState>>,
    body: Option<Json<SyncPlan>>,
) -> Result<Json<SyncResult>, (StatusCode, Json<Value>)> {
    let plan = match body {
        Some(Json(plan)) => plan,
        None => build_sync_plan(&state)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "compile failed" }))))?,
    };

    let result = sync_to_nerve(&state.nerve, &plan).await;
    state.refresh_l7_from_store().await;
    Ok(Json(result))
}

async fn nerve_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let status = state.nerve.get_status().await.map_err(map_nerve_err)?;
    let policy = state
        .nerve
        .get_policy_status()
        .await
        .map_err(map_nerve_err)?;
    let blacklist = state.nerve.get_blacklist().await.map_err(map_nerve_err)?;

    Ok(Json(json!({
        "status": status,
        "policy": policy,
        "blacklist": blacklist,
    })))
}

async fn nerve_observer_live(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .nerve
        .get_observer_state()
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .nerve
        .get_sessions(100)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let id = q.get("id").cloned().unwrap_or_default();
    if id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing id" })),
        ));
    }
    state
        .nerve
        .get_session_graph(&id)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_baseline(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .nerve
        .get_baseline()
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_anomalies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .nerve
        .get_anomalies(100)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_policy_hits(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .nerve
        .get_policy_hits(100)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_explain(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let index = q
        .get("index")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    state
        .nerve
        .explain_anomaly(index)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_journeys(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(50);
    let trace_id = q.get("trace_id").cloned();
    let signal_only = q
        .get("signal_only")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let since_minutes = q
        .get("since_minutes")
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|m| *m > 0);
    state
        .nerve
        .get_journeys(limit, trace_id.as_deref(), signal_only, since_minutes)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_journey(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let id = q.get("id").cloned().unwrap_or_default();
    if id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing id" })),
        ));
    }
    state
        .nerve
        .get_journey(&id)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

async fn nerve_observer_trace_genealogy(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let trace_id = q.get("trace_id").cloned().unwrap_or_default();
    if trace_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "missing trace_id" })),
        ));
    }
    state
        .nerve
        .get_trace_genealogy(&trace_id)
        .await
        .map(Json)
        .map_err(map_nerve_err)
}

#[derive(Deserialize)]
struct PolicyIpBody {
    ip: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    /// `temporary` = runtime ban only; `permanent` writes ip-blacklist plugin config.
    #[serde(default)]
    kind: Option<String>,
    /// On unban, also remove from plugin config (permanent bans).
    #[serde(default)]
    remove_from_config: Option<bool>,
    /// Temporary ban expiry (RFC3339); mutually exclusive with `ttl_secs` (this wins).
    #[serde(default)]
    expires_at: Option<String>,
    /// Temporary ban TTL (seconds); usable when `expires_at` is omitted.
    #[serde(default)]
    ttl_secs: Option<u64>,
}

fn resolve_temp_expires_at(body: &PolicyIpBody) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Some(raw) = body.expires_at.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            return Some(dt.with_timezone(&chrono::Utc));
        }
    }
    body.ttl_secs
        .filter(|s| *s > 0)
        .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64))
}

fn policy_kind(body: &PolicyIpBody) -> &str {
    match body.kind.as_deref() {
        Some("permanent") => "permanent",
        _ => "temporary",
    }
}

async fn policy_deny(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PolicyIpBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let kind = policy_kind(&body);
    let source = body.source.as_deref().unwrap_or("api");

    if kind == "permanent" {
        add_ip_to_declarative_blacklist(&state, &body.ip, source)
            .await
            .map_err(map_config_err)?;
        state
            .nerve
            .deny_ip(&body.ip)
            .await
            .map_err(map_nerve_err)?;
        return Ok(Json(json!({
            "ip": body.ip,
            "status": "denied",
            "kind": "permanent",
            "source": source,
        })));
    }

    let expires_at = resolve_temp_expires_at(&body);

    state
        .config
        .add_runtime_deny(
            &body.ip,
            source,
            body.reason.as_deref(),
            Some(source),
            expires_at,
        )
        .await
        .map_err(map_config_err)?;

    state
        .nerve
        .deny_ip(&body.ip)
        .await
        .map_err(map_nerve_err)?;

    Ok(Json(json!({
        "ip": body.ip,
        "status": "denied",
        "kind": "temporary",
        "source": source,
        "expires_at": expires_at.map(|t| t.to_rfc3339()),
    })))
}

async fn policy_allow(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PolicyIpBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let remove_config = body.remove_from_config == Some(true)
        || policy_kind(&body) == "permanent";

    state
        .config
        .remove_runtime_deny(&body.ip)
        .await
        .map_err(map_config_err)?;

    if remove_config {
        remove_ip_from_declarative_blacklist(&state, &body.ip, "api")
            .await
            .map_err(map_config_err)?;
        let plan = build_sync_plan(&state)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "sync compile failed" })),
                )
            })?;
        sync_to_nerve(&state.nerve, &plan).await;
    } else {
        state
            .nerve
            .allow_ip(&body.ip)
            .await
            .map_err(map_nerve_err)?;
    }

    Ok(Json(json!({
        "ip": body.ip,
        "status": "allowed",
        "removed_from_config": remove_config,
    })))
}

async fn add_ip_to_declarative_blacklist(
    state: &AppState,
    ip: &str,
    operator: &str,
) -> Result<(), nerve_waf_store::ConfigError> {
    let mut config = state.config.load_config().await?;
    let route_idx = config
        .routes
        .iter()
        .position(|r| r.id == "manual-blacklist")
        .or_else(|| {
            config.routes.iter().position(|r| {
                r.enabled
                    && r.layer != "l7"
                    && r.plugins
                        .iter()
                        .any(|p| p.enabled && p.name == "ip-blacklist")
            })
        });

    let Some(route_idx) = route_idx else {
        return Err(nerve_waf_store::ConfigError::Db(
            "no ip-blacklist plugin route found to add permanent deny".into(),
        ));
    };

    let mut updated = false;
    for plugin in &mut config.routes[route_idx].plugins {
        if plugin.name != "ip-blacklist" || !plugin.enabled {
            continue;
        }
        let ips = plugin
            .config
            .as_object_mut()
            .and_then(|o| {
                if !o.contains_key("ips") {
                    o.insert("ips".into(), json!([]));
                }
                o.get_mut("ips")
            })
            .and_then(|v| v.as_array_mut());

        if let Some(ips) = ips {
            if !ips.iter().any(|v| v.as_str() == Some(ip)) {
                ips.push(json!(ip));
                updated = true;
            }
        }
    }

    if !updated {
        return Ok(());
    }

    state.config.save_config(&config, operator).await
}

async fn remove_ip_from_declarative_blacklist(
    state: &AppState,
    ip: &str,
    operator: &str,
) -> Result<(), nerve_waf_store::ConfigError> {
    let mut config = state.config.load_config().await?;
    let mut updated = false;

    for route in &mut config.routes {
        for plugin in &mut route.plugins {
            if plugin.name != "ip-blacklist" {
                continue;
            }
            if let Some(ips) = plugin.config.get_mut("ips").and_then(|v| v.as_array_mut()) {
                let before = ips.len();
                ips.retain(|v| v.as_str() != Some(ip));
                if ips.len() != before {
                    updated = true;
                }
            }
        }
    }

    if updated {
        state.config.save_config(&config, operator).await?;
    }
    Ok(())
}

async fn policy_blacklist(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let config = state
        .config
        .load_config()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let runtime = state
        .config
        .list_runtime_denies()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut entries = Vec::new();
    let mut seen = HashSet::new();

    for route in &config.routes {
        if !route.enabled {
            continue;
        }
        for plugin in &route.plugins {
            if !plugin.enabled || plugin.name != "ip-blacklist" {
                continue;
            }
            if let Some(ips) = plugin.config.get("ips").and_then(|v| v.as_array()) {
                for ip in ips {
                    if let Some(ip_str) = ip.as_str() {
                        if seen.insert(ip_str.to_string()) {
                            entries.push(json!({
                                "ip": ip_str,
                                "action": "deny",
                                "kind": "permanent",
                                "source": "declarative",
                                "route_id": route.id,
                            }));
                        }
                    }
                }
            }
        }
    }

    for deny in runtime {
        if seen.insert(deny.ip.clone()) {
            entries.push(json!({
                "ip": deny.ip,
                "action": "deny",
                "kind": "temporary",
                "source": deny.source,
                "reason": deny.reason,
                "expires_at": deny.expires_at,
            }));
        } else {
            entries.push(json!({
                "ip": deny.ip,
                "action": "deny",
                "kind": "temporary",
                "source": deny.source,
                "reason": deny.reason,
                "expires_at": deny.expires_at,
                "overlay": true,
            }));
        }
    }

    Ok(Json(json!({
        "entries": entries,
        "total": entries.len(),
    })))
}

async fn list_runtime_denies(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let denies = state
        .config
        .list_runtime_denies()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "denies": denies, "total": denies.len() })))
}

fn map_nerve_err(err: nerve_waf_client::NerveClientError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(json!({ "error": err.to_string() })),
    )
}

fn map_config_err(err: nerve_waf_store::ConfigError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
}
