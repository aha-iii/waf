//! WAF Console 独立进程：静态 UI + `/api/*` 反代到 nerve-waf-api。

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use clap::Parser;
use nerve_waf_api::bootstrap;
use nerve_waf_api::console_static;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::EnvFilter;

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

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-console", about = "Nerve WAF Console (static UI + API proxy)")]
struct Args {
    #[arg(long, env = "WAF_CONSOLE_PORT")]
    port: Option<u16>,
    #[arg(long, env = "CONSOLE_PORT")]
    console_port: Option<u16>,
    #[arg(long, env = "WAF_API_UPSTREAM")]
    api_upstream: Option<String>,
}

fn default_port() -> u16 {
    std::env::var("WAF_CONSOLE_PORT")
        .or_else(|_| std::env::var("CONSOLE_PORT"))
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8782)
}

fn default_api_upstream() -> String {
    std::env::var("WAF_API_UPSTREAM")
        .unwrap_or_else(|_| "http://127.0.0.1:8781".into())
        .trim_end_matches('/')
        .to_string()
}

fn build_upstream_uri(base: &str, path_and_query: &str) -> Result<Uri, String> {
    let base = base.trim_end_matches('/');
    let pq = if path_and_query.is_empty() {
        "/".to_string()
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
        return Err(format!("api upstream must be absolute URL with host: {full}"));
    }
    Ok(uri)
}

fn filter_request_headers(src: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        if HOP_BY_HOP.iter().any(|h| *h == name.as_str()) {
            continue;
        }
        out.insert(name.clone(), value.clone());
    }
    out
}

fn filter_response_headers(src: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in src.iter() {
        if HOP_BY_HOP.iter().any(|h| *h == name.as_str()) {
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

async fn proxy_api(req: Request) -> Response {
    let base = default_api_upstream();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let upstream = match build_upstream_uri(&base, &path_and_query) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("api proxy uri: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "application/json")],
                r#"{"error":"invalid api upstream"}"#,
            )
                .into_response();
        }
    };

    let method = method_to_reqwest(req.method());
    let headers = filter_request_headers(req.headers());
    let body_bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("api proxy read body: {e}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("api proxy client: {e}");
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
                    tracing::warn!("api proxy read upstream body: {e}");
                    return (
                        StatusCode::BAD_GATEWAY,
                        [(header::CONTENT_TYPE, "application/json")],
                        r#"{"error":"api upstream body read failed"}"#,
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
            tracing::warn!("api proxy to {base} failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "application/json")],
                format!(
                    r#"{{"error":"waf-api unreachable","detail":"{}","hint":"check WAF_API_UPSTREAM and nerve-waf-api"}}"#,
                    e.to_string().replace('"', "'")
                ),
            )
                .into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    bootstrap::load_env_files();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "nerve_waf_api=info");
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let port = args
        .port
        .or(args.console_port)
        .unwrap_or_else(default_port);
    let upstream = args
        .api_upstream
        .unwrap_or_else(default_api_upstream);

    let Some(dir) = console_static::resolve_console_dir() else {
        eprintln!(
            "nerve-waf-console: no index.html under WAF_CONSOLE_DIR or /opt/nerve-waf/console"
        );
        std::process::exit(1);
    };

    tracing::info!(
        port,
        upstream = %upstream,
        dir = %dir.display(),
        "nerve-waf-console listening (static UI + /api proxy)"
    );

    let index = dir.join("index.html");
    let static_service = ServeDir::new(dir).not_found_service(ServeFile::new(index));

    let app = Router::new()
        .route("/api", any(proxy_api))
        .route("/api/{*rest}", any(proxy_api))
        .fallback_service(static_service);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
