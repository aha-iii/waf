//! 将 Console `/api/alerts/*` 反向代理到 Nerve-alert。
//!
//! 开发态由 Vite 代理到 `:8790`；生产态由 waf-api 同端口托管时，
//! 若无此代理，请求会落入 SPA fallback（PUT → 405，GET → index.html）。

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use std::sync::Arc;
use std::time::Duration;

use crate::controlplane::shared::AppState;

const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
];

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/alerts", any(proxy_alerts))
        .route("/api/alerts/{*rest}", any(proxy_alerts))
}

fn alert_base_url() -> String {
    std::env::var("NERVE_ALERT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8790".into())
        .trim_end_matches('/')
        .to_string()
}

/// 将传入的 Console 路径拼到 Nerve-alert 基址；保留 query。
pub fn build_upstream_uri(base: &str, path_and_query: &str) -> Result<Uri, String> {
    let base = base.trim_end_matches('/');
    let pq = if path_and_query.is_empty() {
        "/api/alerts".to_string()
    } else if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    let full = format!("{base}{pq}");
    let uri = full
        .parse::<Uri>()
        .map_err(|e| format!("invalid upstream uri {full}: {e}"))?;
    if uri.scheme().is_none() || uri.host().is_none() {
        return Err(format!("alert upstream must be absolute URL with host: {full}"));
    }
    Ok(uri)
}

fn filter_request_headers(src: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        let lower = name.as_str();
        if HOP_BY_HOP.iter().any(|h| *h == lower) {
            continue;
        }
        out.insert(name.clone(), value.clone());
    }
    out
}

fn filter_response_headers(src: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        let lower = name.as_str();
        if HOP_BY_HOP.iter().any(|h| *h == lower) {
            continue;
        }
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out.insert(n, v);
        }
    }
    out
}

fn method_to_reqwest(m: &Method) -> reqwest::Method {
    reqwest::Method::from_bytes(m.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

async fn proxy_alerts(req: Request) -> Response {
    let base = alert_base_url();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let upstream = match build_upstream_uri(&base, &path_and_query) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("alert proxy uri: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "application/json")],
                r#"{"error":"invalid alert upstream"}"#,
            )
                .into_response();
        }
    };

    let method = method_to_reqwest(req.method());
    let headers = filter_request_headers(req.headers());
    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("alert proxy read body: {e}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("alert proxy client: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut builder = client.request(method, upstream.to_string());
    for (k, v) in headers.iter() {
        if let Ok(val) = v.to_str() {
            builder = builder.header(k.as_str(), val);
        }
    }
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    match builder.send().await {
        Ok(res) => {
            let status =
                StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let resp_headers = filter_response_headers(res.headers());
            let bytes = match res.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("alert proxy read upstream body: {e}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        [(header::CONTENT_TYPE, "application/json")],
                        r#"{"error":"alert upstream body read failed"}"#,
                    )
                        .into_response();
                }
            };
            let mut response = Response::new(Body::from(bytes));
            *response.status_mut() = status;
            *response.headers_mut() = resp_headers;
            response
        }
        Err(e) => {
            tracing::warn!("alert proxy to {base} failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "application/json")],
                format!(
                    r#"{{"error":"nerve-alert unreachable","detail":"{}","hint":"check NERVE_ALERT_URL and nerve-alert-api"}}"#,
                    e.to_string().replace('"', "'")
                ),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_upstream_preserves_path_and_query() {
        let u = build_upstream_uri(
            "http://127.0.0.1:8790/",
            "/api/alerts/config?product=waf",
        )
        .unwrap();
        assert_eq!(u.path(), "/api/alerts/config");
        assert_eq!(u.query(), Some("product=waf"));
        assert_eq!(u.scheme_str(), Some("http"));
    }

    #[test]
    fn build_upstream_acks_nested_path() {
        let u = build_upstream_uri(
            "http://nerve-alert:8790",
            "/api/alerts/logs/abc/ack",
        )
        .unwrap();
        assert_eq!(u.path(), "/api/alerts/logs/abc/ack");
    }

    #[test]
    fn build_upstream_rejects_empty_base() {
        assert!(build_upstream_uri("", "/api/alerts/config").is_err());
    }
}
