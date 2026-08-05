mod list;
mod watch;

use std::time::Duration;

use crate::dataplane::runtime::{EdgeRuntime, SharedRuntime};

pub use list::fetch_dataplane;
pub use watch::{ack_generation, watch_dataplane};

fn sync_mode() -> String {
    std::env::var("WAF_EDGE_SYNC_MODE")
        .unwrap_or_else(|_| "watch".into())
        .trim()
        .to_lowercase()
}

fn poll_interval_sec() -> u64 {
    std::env::var("WAF_EDGE_POLL_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
}

pub fn spawn_sync_loop(api: String, runtime: SharedRuntime, waf: std::sync::Arc<nerve_waf_embed::EmbeddedWaf>) {
    tokio::spawn(async move {
        let mode = sync_mode();
        let mut last_gen = 0u64;
        let poll = Duration::from_secs(poll_interval_sec());

        loop {
            let result = if mode == "watch" {
                match watch_dataplane(&api, last_gen, 30).await {
                    Ok(Some(snap)) => Ok(snap),
                    Ok(None) => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!("watch failed, fallback list: {e}");
                        fetch_dataplane(&api).await
                    }
                }
            } else {
                fetch_dataplane(&api).await
            };

            match result {
                Ok(snap) => {
                    let gen = snap.generation;
                    let n = snap.routes.len();
                    let u = snap.upstreams.len();
                    waf.apply_global_overlay(snap.global.clone());
                    *runtime.write().await = EdgeRuntime::from_snapshot(snap);
                    if gen > last_gen {
                        last_gen = gen;
                        ack_generation(&api, gen).await;
                    }
                    tracing::info!(generation = gen, routes = n, upstreams = u, "dataplane loaded");
                }
                Err(e) => tracing::warn!("dataplane refresh: {e}"),
            }

            if mode != "watch" {
                tokio::time::sleep(poll).await;
            }
        }
    });
}
