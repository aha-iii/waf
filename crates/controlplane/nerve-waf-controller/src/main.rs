//! Binary entry for `nerve-waf-controller` (Axum server, config/env bootstrap).

use axum::Router;
use clap::Parser;
use nerve_waf_controller::controlplane;
use nerve_waf_controller::dataplane;
use nerve_waf_controller::logging;
use nerve_waf_controller::phase11_middleware;
use nerve_waf_controller::bootstrap::{self, BootstrapOptions};
use nerve_waf_controller::trace_middleware;
use nerve_waf_sdk::WafFileConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use axum::http::HeaderValue;
use axum::middleware;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-controller", about = "Nerve-waf Control API")]
struct Args {
    #[arg(long, env = "WAF_CONFIG")]
    config: Option<PathBuf>,
    #[arg(long, env = "PORT")]
    port: Option<u16>,
    #[arg(long, env = "NERVE_URL")]
    nerve_url: Option<String>,
    #[arg(long, env = "NERVE_WAF_DATA")]
    data_dir: Option<String>,
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,
}

/// CORS: `NERVE_CORS_ORIGINS` is a comma-separated allowlist.
/// - unset / empty / `off`: reflect no Origin (prod relies on APISIX `cors` plugin).
/// - `*`: allow any Origin (local debug only).
/// - otherwise: allowlist (e.g. `https://waf.example.com`).
fn build_cors_layer() -> CorsLayer {
    let raw = std::env::var("NERVE_CORS_ORIGINS").unwrap_or_default();
    let raw = raw.trim();
    let layer = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any);
    if raw == "*" {
        return layer.allow_origin(Any);
    }
    if raw.is_empty() || raw.eq_ignore_ascii_case("off") {
        return layer.allow_origin(AllowOrigin::list(Vec::<HeaderValue>::new()));
    }
    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| HeaderValue::from_str(s).ok())
        .collect();
    if origins.is_empty() {
        return layer.allow_origin(AllowOrigin::list(Vec::<HeaderValue>::new()));
    }
    layer.allow_origin(AllowOrigin::list(origins))
}

#[tokio::main]
async fn main() {
    bootstrap::load_env_files();

    let args = Args::parse();
    let file = WafFileConfig::load_or_default(args.config.as_deref());
    let node = &file.waf;

    logging::configure(&file.logging);

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            format!(
                "nerve_waf_controller={},nerve_waf={},waf.server={}",
                file.logging.level, file.logging.level, file.logging.level
            ),
        );
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(
                    format!("nerve_waf_controller={}", file.logging.level)
                        .parse()
                        .unwrap(),
                )
                .add_directive(
                    format!("waf.server={}", file.logging.level)
                        .parse()
                        .unwrap(),
                ),
        )
        .init();

    let port = args.port.unwrap_or(node.api.port);
    let mut opts = BootstrapOptions::from_env_and_config(args.config.clone());
    if let Some(d) = args.data_dir {
        opts.data_dir = Some(PathBuf::from(d));
    }
    if let Some(url) = args.database_url.filter(|u| !u.trim().is_empty()) {
        opts.database_url = Some(url);
    }
    if args.nerve_url.is_some() {
        opts.nerve_url = args.nerve_url;
    }

    let database_url_set = opts.database_url.is_some();
    let state = bootstrap::build_app_state(opts).await;
    let config_backend = state.config.backend_name();
    let event_backend = state.events.backend_name();

    nerve_waf_controller::events::spawn_nerve_poller(state.clone());

    if std::env::var("NERVE_CRS_AUTO_UPDATE").ok().as_deref() == Some("1") {
        let st = state.clone();
        let hours: u64 = std::env::var("NERVE_CRS_AUTO_UPDATE_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(168);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(hours * 3600)).await;
                let platform = st.platform.clone();
                let version = std::env::var("NERVE_CRS_VERSION").unwrap_or_else(|_| "v4.21.0".into());
                let version_log = version.clone();
                match tokio::task::spawn_blocking(move || platform.crs_update_latest(&version, 2)).await {
                    Ok(Ok(_)) => {
                        st.refresh_l7_from_store().await;
                        tracing::info!("CRS auto-update completed (version={version_log})");
                    }
                    Ok(Err(e)) => tracing::warn!("CRS auto-update failed: {e}"),
                    Err(e) => tracing::warn!("CRS auto-update task error: {e}"),
                }
            }
        });
        tracing::info!("CRS auto-update enabled (every {hours}h, set NERVE_CRS_AUTO_UPDATE=1)");
    }

    let router = Router::new()
        .merge(controlplane::controlplane_router())
        .merge(dataplane::dataplane_router())
        .layer(axum::middleware::from_fn(trace_middleware::trace_middleware))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            phase11_middleware::license_enforce_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            phase11_middleware::rbac_enforce_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            phase11_middleware::auth_enforce_middleware,
        ))
        .layer(build_cors_layer())
        .with_state(state.clone());

    let attach_console = std::env::var("WAF_CONSOLE_ATTACH")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let app = if attach_console {
        tracing::info!("WAF_CONSOLE_ATTACH=1: Console static UI on API port {port}");
        nerve_waf_controller::console_static::attach_console_fallback(router)
    } else {
        tracing::info!("WAF Console separated (set WAF_CONSOLE_ATTACH=1 to co-locate static UI on API port)");
        router
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(
        port,
        config_backend,
        event_backend,
        database = if database_url_set { "postgresql" } else { "file" },
        console_attach = attach_console,
        "nerve-waf-controller listening (controlplane + dataplane)"
    );
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
