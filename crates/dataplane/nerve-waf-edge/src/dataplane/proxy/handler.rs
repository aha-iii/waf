use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::dataplane::inspect::{build_embed_request, inspect_for_site_blocking, SharedInspectCache};
use crate::dataplane::runtime::{pick_upstream_url, SharedRuntime};
use nerve_waf_embed::EmbeddedWaf;

use super::block_page::{self, BlockPageFields};
use super::events::report_block_event;
use super::client_ip::resolve_client_ip;

/// GitLab 推送 / 工件与前置 TLS `client_max_body_size` 对齐（256m）。
fn max_body_bytes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("WAF_EDGE_MAX_BODY_MB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(256)
            .saturating_mul(1024 * 1024)
    })
}

/// 正式入口链：TLS → ddos → waf-edge → APISIX。设置后忽略站点 origins，统一回源内网 APISIX。
fn force_upstream_base() -> Option<String> {
    static V: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        std::env::var("WAF_EDGE_FORCE_UPSTREAM")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
    .clone()
}

fn passthrough_unmatched() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        matches!(
            std::env::var("WAF_EDGE_PASSTHROUGH_UNMATCHED")
                .ok()
                .as_deref(),
            Some("1") | Some("true") | Some("yes")
        )
    })
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn smuggle_diag_headers(headers: &HeaderMap) -> String {
    let te = header_str(headers, "transfer-encoding").unwrap_or_default();
    let cl = header_str(headers, "content-length").unwrap_or_default();
    let x_orig = header_str(headers, "x-original-url").unwrap_or_default();
    let x_rewrite = header_str(headers, "x-rewrite-url").unwrap_or_default();
    let host_count = headers
        .get_all(header::HOST)
        .into_iter()
        .count();
    format!(
        "te={te:?} cl={cl:?} x-original-url={x_orig:?} x-rewrite-url={x_rewrite:?} host_count={host_count}"
    )
}

fn edge_timing_log_all() -> bool {
    static ALL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALL.get_or_init(|| {
        matches!(
            std::env::var("NERVE_EDGE_TIMING_ALL").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        )
    })
}

fn log_edge_request_timing(
    method: &Method,
    path_q: &str,
    host: &str,
    blocked: bool,
    status: Option<u16>,
    body_us: u64,
    route_us: u64,
    inspect_us: u64,
    upstream_us: u64,
    total_us: u64,
    timing: Option<&nerve_waf_embed::InspectTimingReport>,
) {
    const THRESHOLD_US: u64 = 50_000;
    if !edge_timing_log_all() && total_us < THRESHOLD_US {
        return;
    }
    let nodes = timing.map(|t| {
        t.nodes
            .iter()
            .map(|n| format!("{}={}us", n.n, n.us))
            .collect::<Vec<_>>()
            .join(",")
    });
    let inspect_path = timing.and_then(|t| t.path.as_deref());
    let inspect_nodes_us = timing.map(|t| t.total_us);
    tracing::info!(
        method = %method,
        uri = %path_q,
        host = %host,
        blocked,
        status = status.unwrap_or(0),
        body_us,
        route_us,
        inspect_us,
        inspect_nodes_us,
        inspect_path,
        inspect_nodes = nodes.as_deref().unwrap_or(""),
        upstream_us,
        total_us,
        "edge request timing"
    );
}

fn log_edge_block(
    inspect: &nerve_waf_embed::EmbedInspectResponse,
    method: &Method,
    path_q: &str,
    host: &str,
    src_ip: &str,
    headers: &HeaderMap,
) {
    let rule_id = inspect.rule_id.as_deref().unwrap_or("");
    let is_smuggle = rule_id.contains("smuggle") || rule_id.contains("transfer-encoding");
    if is_smuggle {
        tracing::warn!(
            rule_id = %rule_id,
            rule_name = ?inspect.rule_name,
            evidence = ?inspect.payload_sample,
            param_path = ?inspect.param_path,
            method = %method,
            host = %host,
            uri = %path_q,
            src_ip = %src_ip,
            smuggle_headers = %smuggle_diag_headers(headers),
            "edge blocked smuggle-related rule"
        );
    } else {
        tracing::info!(
            rule_id = %rule_id,
            method = %method,
            uri = %path_q,
            src_ip = %src_ip,
            "edge blocked request"
        );
    }
}

#[derive(Clone)]
pub struct EdgeState {
    pub runtime: SharedRuntime,
    pub rr: Arc<Mutex<HashMap<String, usize>>>,
    pub waf: Arc<EmbeddedWaf>,
    pub inspect_cache: SharedInspectCache,
    pub client: reqwest::Client,
    pub api: String,
}

pub async fn proxy_handler(
    State(state): State<EdgeState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> Response {
    let t_total = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = match axum::body::to_bytes(req.into_body(), max_body_bytes()).await {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "body too large").into_response();
        }
    };
    let body_us = t_total.elapsed().as_micros() as u64;
    let body_str = String::from_utf8_lossy(&body).into_owned();
    let peer_ip = peer.ip().to_string();
    let src_ip = resolve_client_ip(&headers, &peer);
    if src_ip != peer_ip {
        tracing::debug!(
            client_ip = %src_ip,
            peer_ip = %peer_ip,
            "resolved client IP from forwarded headers"
        );
    }
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let path_q = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();

    let force_base = force_upstream_base();

    // 未登记站点：全站入口链下仍转发到 APISIX（仅过进程，不做站点级 inspect）
    let matched = {
        let rt = state.runtime.read().await;
        rt.match_route(&host).cloned()
    };
    if matched.is_none() {
        let route_us = t_total.elapsed().as_micros() as u64 - body_us;
        if let (true, Some(base)) = (passthrough_unmatched(), force_base.as_ref()) {
            return forward_upstream(
                &state,
                &method,
                &host,
                &path_q,
                &headers,
                &body,
                base,
                body_us,
                route_us,
                0,
                t_total,
            )
            .await;
        }
        return (StatusCode::NOT_FOUND, format!("no route for Host: {host}")).into_response();
    }
    let route = matched.expect("matched");

    let upstream_url = if let Some(base) = force_base.clone() {
        base
    } else {
        let rt = state.runtime.read().await;
        let up = match rt.upstream(&route.upstream_id) {
            Some(u) => u.clone(),
            None => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("upstream `{}` not found", route.upstream_id),
                )
                    .into_response();
            }
        };
        drop(rt);
        let mut rr = state.rr.lock().await;
        match pick_upstream_url(&up, &mut rr) {
            Some(u) => u,
            None => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("upstream `{}` unavailable", route.upstream_id),
                )
                    .into_response();
            }
        }
    };
    let route_us = t_total.elapsed().as_micros() as u64 - body_us;

    let headers_json = headers_to_json(&headers);
    let embed_req = build_embed_request(
        method.to_string(),
        host.clone(),
        path_q.clone(),
        headers_json,
        body_str.clone(),
        src_ip.clone(),
        ua.clone(),
    );
    let waf = Arc::clone(&state.waf);
    let inspect_cache = Arc::clone(&state.inspect_cache);
    let site_id = route.site_id.clone();
    let (site_snap, global, generation) = {
        let rt = state.runtime.read().await;
        (
            rt.site_inspect(&route.site_id).cloned(),
            rt.global().clone(),
            rt.generation(),
        )
    };
    let inspect = match tokio::task::spawn_blocking(move || {
        inspect_for_site_blocking(
            &waf,
            &inspect_cache,
            &site_id,
            generation,
            site_snap,
            global,
            embed_req,
            nerve_waf_embed::InspectPreparedOptions::default(),
        )
    })
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "edge inspect task failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "inspect failed").into_response();
        }
    };
    let inspect_us = t_total.elapsed().as_micros() as u64 - body_us - route_us;

    if inspect.blocked {
        log_edge_block(&inspect, &method, &path_q, &host, &src_ip, &headers);
        log_edge_request_timing(
            &method,
            &path_q,
            &host,
            true,
            Some(StatusCode::FORBIDDEN.as_u16()),
            body_us,
            route_us,
            inspect_us,
            0,
            t_total.elapsed().as_micros() as u64,
            inspect.timing.as_ref(),
        );
        let event_id = block_page::new_event_id();
        let fields = BlockPageFields {
            event_id: event_id.clone(),
            action: inspect.action.clone(),
            rule_id: inspect.rule_id.clone().unwrap_or_default(),
            site_id: route.site_id.clone(),
        };
        let client = state.client.clone();
        let api = state.api.clone();
        let site_id = route.site_id.clone();
        let method_s = method.to_string();
        let host_s = host.clone();
        let path_q_s = path_q.clone();
        let src_ip_s = src_ip.clone();
        let ua_s = ua;
        let inspect_snap = inspect.clone();
        let headers_snap = headers.clone();
        let body_snap = body_str.clone();
        tokio::spawn(async move {
            report_block_event(
                &client,
                &api,
                &site_id,
                &method_s,
                &host_s,
                &path_q_s,
                &src_ip_s,
                &ua_s,
                &event_id,
                inspect_snap,
                &headers_snap,
                &body_snap,
            )
            .await;
        });
        if block_page::prefers_json(&headers) {
            return (
                StatusCode::FORBIDDEN,
                [(
                    header::CONTENT_TYPE,
                    "application/json; charset=utf-8",
                )],
                block_page::render_json(&fields, inspect.rule_name.as_deref()),
            )
                .into_response();
        }
        return (
            StatusCode::FORBIDDEN,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            block_page::render_html(&fields),
        )
            .into_response();
    }

    forward_upstream(
        &state,
        &method,
        &host,
        &path_q,
        &headers,
        &body,
        &upstream_url,
        body_us,
        route_us,
        inspect_us,
        t_total,
    )
    .await
}

async fn forward_upstream(
    state: &EdgeState,
    method: &Method,
    host: &str,
    path_q: &str,
    headers: &HeaderMap,
    body: &axum::body::Bytes,
    upstream_base: &str,
    body_us: u64,
    route_us: u64,
    inspect_us: u64,
    t_total: Instant,
) -> Response {
    let target = join_upstream_url(upstream_base, path_q);
    let mut fwd = state.client.request(method_to_reqwest(method), target);
    fwd = copy_forward_headers(fwd, headers, host);
    if !body.is_empty() {
        fwd = fwd.body(body.to_vec());
    }
    let t_upstream = Instant::now();
    match fwd.send().await {
        Ok(upstream_resp) => {
            let status = upstream_resp.status();
            let resp_headers = upstream_resp.headers().clone();
            let bytes = upstream_resp.bytes().await.unwrap_or_default();
            let upstream_us = t_upstream.elapsed().as_micros() as u64;
            log_edge_request_timing(
                method,
                path_q,
                host,
                false,
                Some(status.as_u16()),
                body_us,
                route_us,
                inspect_us,
                upstream_us,
                t_total.elapsed().as_micros() as u64,
                None,
            );
            response_from_upstream(status, &resp_headers, bytes)
        }
        Err(e) => {
            tracing::warn!(error = %e, host = %host, "upstream error");
            log_edge_request_timing(
                method,
                path_q,
                host,
                false,
                Some(StatusCode::BAD_GATEWAY.as_u16()),
                body_us,
                route_us,
                inspect_us,
                t_upstream.elapsed().as_micros() as u64,
                t_total.elapsed().as_micros() as u64,
                None,
            );
            (StatusCode::BAD_GATEWAY, format!("upstream error: {e}")).into_response()
        }
    }
}

fn response_from_upstream(
    status: reqwest::StatusCode,
    resp_headers: &reqwest::header::HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let mut out = Response::builder().status(status.as_u16());
    if let Some(h) = out.headers_mut() {
        copy_response_headers(h, resp_headers);
    }
    out.body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub async fn health_handler(State(state): State<EdgeState>) -> Json<serde_json::Value> {
    let rt = state.runtime.read().await;
    Json(serde_json::json!({
        "ok": true,
        "generation": rt.generation(),
        "checksum": rt.snap.checksum,
        "routes": rt.snap.routes.len(),
        "upstreams": rt.snap.upstreams.len(),
        "site_inspect_sites": rt.snap.site_inspect.len(),
        "force_upstream": force_upstream_base(),
        "passthrough_unmatched": passthrough_unmatched(),
    }))
}

fn method_to_reqwest(m: &Method) -> reqwest::Method {
    reqwest::Method::from_bytes(m.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

fn join_upstream_url(base: &str, path_q: &str) -> String {
    let base = base.trim_end_matches('/');
    if path_q.starts_with('/') {
        format!("{base}{path_q}")
    } else {
        format!("{base}/{path_q}")
    }
}

fn headers_to_json(h: &HeaderMap) -> String {
    let mut map = serde_json::Map::new();
    for (k, v) in h.iter() {
        if let Ok(s) = v.to_str() {
            map.insert(k.as_str().to_string(), serde_json::Value::String(s.into()));
        }
    }
    serde_json::Value::Object(map).to_string()
}

fn copy_forward_headers(
    mut req: reqwest::RequestBuilder,
    src: &HeaderMap,
    host: &str,
) -> reqwest::RequestBuilder {
    for (k, v) in src.iter() {
        let name = k.as_str();
        if matches!(
            name,
            "host" | "connection" | "transfer-encoding" | "content-length"
                | "accept-encoding"
        ) {
            continue;
        }
        if let Ok(s) = v.to_str() {
            req = req.header(name, s);
        }
    }
    req.header(header::HOST.as_str(), host)
}

fn copy_response_headers(dst: &mut HeaderMap, src: &reqwest::header::HeaderMap) {
    for (k, v) in src.iter() {
        if should_skip_response_header(k.as_str()) {
            continue;
        }
        if let (Ok(name), Ok(val)) = (
            header::HeaderName::from_bytes(k.as_str().as_bytes()),
            header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            dst.insert(name, val);
        }
    }
}

/// 仅去掉 hop-by-hop；保留 Content-Type / Content-Length / Content-Encoding（client 已 no_gzip，上游多为明文 UTF-8）
fn should_skip_response_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "transfer-encoding"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "upgrade"
    )
}
