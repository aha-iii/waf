//! Phase 11 — RBAC enforcement + License metering middleware.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use nerve_waf_auth::AuthMethod;
use std::sync::Arc;

use crate::AppState;

fn is_api_route(path: &str) -> bool {
    path.starts_with("/api/")
}

fn is_auth_public(path: &str) -> bool {
    if is_dataplane_pull(path) {
        return true;
    }
    // /api/inspect must authenticate (anti rule-oracle); login page only allows health/auth public surface.
    matches!(
        path,
        "/api/health"
            | "/api/auth/status"
            | "/api/auth/config"
            | "/api/auth/login"
            | "/api/public/demo"
    )
}

/// Edge dataplane pulls config (Gateway / nerve-waf-agent); not Console login session.
fn is_dataplane_pull(path: &str) -> bool {
    matches!(
        path,
        "/api/waf/edge/dataplane"
            | "/api/waf/edge/watch"
            | "/api/waf/edge/status"
            | "/api/waf/edge/ack"
            | "/api/waf/edge/events"
            | "/api/gateway/dataplane"
    )
}

fn extract_credentials(req: &Request<Body>) -> (Option<&str>, Option<&str>) {
    let api_key = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok());
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim);
    (api_key, bearer)
}

pub async fn auth_enforce_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.auth.is_enabled() {
        return Ok(next.run(req).await);
    }
    if is_auth_public(req.uri().path()) {
        return Ok(next.run(req).await);
    }
    let (api_key, bearer) = extract_credentials(&req);
    let method = state.auth.authenticate(api_key, bearer).await;
    if method == AuthMethod::None {
        Err(StatusCode::UNAUTHORIZED)
    } else {
        Ok(next.run(req).await)
    }
}

pub async fn rbac_enforce_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !is_api_route(req.uri().path()) {
        return Ok(next.run(req).await);
    }
    if is_dataplane_pull(req.uri().path()) {
        return Ok(next.run(req).await);
    }
    let cfg = state.platform.rbac_config();
    if !cfg.enabled {
        return Ok(next.run(req).await);
    }
    let path = req.uri().path();
    let method = req.method().as_str();
    let (resource, action) = map_route_permission(path, method);
    let (api_key, bearer) = extract_credentials(&req);
    let auth_method = state.auth.authenticate(api_key, bearer).await;
    let role = state.auth.resolve_role(auth_method, api_key, bearer);
    if nerve_waf_rbac::check_permission(&cfg, &role, resource, action) {
        Ok(next.run(req).await)
    } else {
        let _ = bearer;
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn license_enforce_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    if !is_api_route(path) {
        return Ok(next.run(req).await);
    }
    if path == "/api/health" || path == "/api/public/demo" {
        return Ok(next.run(req).await);
    }

    let sites = state.platform.list_sites().len() as u32;
    let agents = if let Some(topo) = &state.topology {
        topo.agent_count().await.unwrap_or(0).max(0) as u32
    } else {
        0
    };
    if state.platform.usage_meter().within_limits(sites, agents) {
        state.platform.usage_meter().record_request();
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::PAYMENT_REQUIRED)
    }
}

fn map_route_permission(path: &str, method: &str) -> (&'static str, &'static str) {
    if path.starts_with("/api/topology") {
        return ("topology", if method == "GET" { "read" } else { "write" });
    }
    if path.starts_with("/api/sync") || path.starts_with("/api/np") {
        return ("policy", "write");
    }
    if path.starts_with("/api/events") {
        return ("events", "read");
    }
    ("*", if method == "GET" { "read" } else { "write" })
}
