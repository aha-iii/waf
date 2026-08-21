//! L7 edge binary: sync snapshot + reverse proxy + inspect.

use axum::{routing::any, Router};
use clap::Parser;
use nerve_waf_embed::EmbeddedWaf;
use nerve_waf_dataplane::WafEdgeSnapshot;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod dataplane;

use dataplane::{health_handler, new_shared_cache, proxy_handler, spawn_sync_loop, EdgeRuntime, EdgeState, SharedRuntime};

fn resolve_plugins_dir(cli: Option<PathBuf>) -> PathBuf {
    if let Some(p) = cli {
        return p;
    }
    for key in ["NERVE_WAF_PLUGINS_DIR", "NERVE_WAF_PLUGINS"] {
        if let Ok(raw) = std::env::var(key) {
            let raw = raw.trim();
            if !raw.is_empty() {
                return PathBuf::from(raw);
            }
        }
    }
    let data_dir = std::env::var("NERVE_WAF_DATA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);
    nerve_waf_plugin_host::resolve_plugins_dir(data_dir.as_deref())
}

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-edge", about = "Nerve WAF L7 edge (inspect + proxy)")]
struct Args {
    #[arg(long, env = "WAF_EDGE_PORT", default_value = "8080")]
    port: u16,
    #[arg(long, env = "WAF_EDGE_API", default_value = "http://127.0.0.1:8781")]
    api: String,
    #[arg(long)]
    plugins_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("nerve_waf_edge=info".parse().unwrap()))
        .init();

    let args = Args::parse();
    let runtime: SharedRuntime = Arc::new(tokio::sync::RwLock::new(EdgeRuntime::from_snapshot(
        WafEdgeSnapshot::default(),
    )));

    let plugins_dir = resolve_plugins_dir(args.plugins_dir);
    tracing::info!("plugins_dir={}", plugins_dir.display());
    let waf = Arc::new(EmbeddedWaf::new(Some(plugins_dir)));

    spawn_sync_loop(args.api.clone(), runtime.clone(), waf.clone());

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .build()
        .expect("http client");

    let state = EdgeState {
        runtime,
        rr: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        waf,
        inspect_cache: new_shared_cache(),
        client,
        api: args.api.clone(),
    };

    let app = Router::new()
        .route("/health", axum::routing::get(health_handler))
        .fallback(any(proxy_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    tracing::info!("nerve-waf-edge listening on http://{addr} (dataplane sync from {})", args.api);
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("serve");
}
