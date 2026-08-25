use axum::{
    body::Body,
    extract::{FromRequestParts, Request},
    http::{HeaderMap, HeaderValue, Response},
    middleware::Next,
};
use nerve_waf_sdk::RequestTrace;
use serde_json::json;
use std::ops::Deref;
use std::time::Instant;

/// Trace context injected by [`trace_middleware`] (via request extensions).
#[derive(Clone, Debug)]
pub struct NerveTrace(pub RequestTrace);

impl Deref for NerveTrace {
    type Target = RequestTrace;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> FromRequestParts<S> for NerveTrace
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let trace = parts
            .extensions
            .get::<RequestTrace>()
            .cloned()
            .unwrap_or_else(|| RequestTrace::new_waf(None, None, None, None, None, None, None));
        async move { Ok(NerveTrace(trace)) }
    }
}

use crate::logging;

fn header_get_ci(headers: &HeaderMap, name: &str) -> Option<String> {
    if let Some(v) = headers.get(name) {
        if let Ok(s) = v.to_str() {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    let lower = name.to_ascii_lowercase();
    headers.iter().find_map(|(k, v)| {
        if k.as_str().eq_ignore_ascii_case(&lower) {
            v.to_str()
                .ok()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        } else {
            None
        }
    })
}

pub async fn trace_middleware(mut req: Request, next: Next) -> Response<Body> {
    let headers = req.headers().clone();
    let trace = RequestTrace::from_http_headers(|name| header_get_ci(&headers, name));
    req.extensions_mut().insert(trace.clone());

    let path = req.uri().path().to_string();
    let method = req.method().to_string();
    let started = Instant::now();

    logging::log_server_at(
        logging::server_level_for_path(&path),
        trace.merge(json!({
            "event": "request_start",
            "method": &method,
            "path": &path,
        })),
    );

    let mut resp = next.run(req).await;
    let latency_ms = started.elapsed().as_millis();

    logging::log_server_at(
        logging::server_level_for_path(&path),
        trace.merge(json!({
            "event": "request_end",
            "method": method,
            "path": path,
            "status": resp.status().as_u16(),
            "latency_ms": latency_ms,
        })),
    );

    // By default do not write Nerve-Trace-Id / traceparent onto external responses (anti info-leak).
    // Debug: NERVE_EXPOSE_TRACE_HEADERS=1.
    let expose_trace = std::env::var("NERVE_EXPOSE_TRACE_HEADERS")
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false);
    if expose_trace {
        for (name, value) in trace.outbound_http_headers() {
            if let Ok(hv) = HeaderValue::from_str(&value) {
                resp.headers_mut().insert(name, hv);
            }
        }
    }
    resp
}

pub fn trace_from_extensions(req: &Request) -> RequestTrace {
    req.extensions()
        .get::<RequestTrace>()
        .cloned()
        .unwrap_or_else(|| RequestTrace::new_waf(None, None, None, None, None, None, None))
}
