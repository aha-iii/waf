//! Edge counts traffic by site+host in three buckets; periodically flush to control plane.

use nerve_waf_embed::EmbedInspectResponse;
use nerve_waf_sdk::TrafficBatchItem;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone, Copy)]
struct Counters {
    total: u64,
    clean: u64,
    observed: u64,
    blocked: u64,
}

#[derive(Debug, Default)]
pub struct TrafficCounters {
    inner: Mutex<HashMap<(String, String), Counters>>,
}

impl TrafficCounters {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(HashMap::new()),
        })
    }

    pub fn record(&self, site_id: &str, host: &str, inspect: &EmbedInspectResponse) {
        let class = classify(inspect);
        let key = (site_id.to_string(), host.trim().to_ascii_lowercase());
        let mut g = self.inner.lock().expect("traffic counters");
        let c = g.entry(key).or_default();
        c.total = c.total.saturating_add(1);
        match class {
            TrafficClass::Clean => c.clean = c.clean.saturating_add(1),
            TrafficClass::Observed => c.observed = c.observed.saturating_add(1),
            TrafficClass::Blocked => c.blocked = c.blocked.saturating_add(1),
        }
    }

    pub fn drain_batch(&self) -> Vec<TrafficBatchItem> {
        let mut g = self.inner.lock().expect("traffic counters");
        if g.is_empty() {
            return Vec::new();
        }
        let snapshot = std::mem::take(&mut *g);
        snapshot
            .into_iter()
            .map(|((site_id, host), c)| TrafficBatchItem {
                site_id,
                host,
                total: c.total,
                clean: c.clean,
                observed: c.observed,
                blocked: c.blocked,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrafficClass {
    Clean,
    Observed,
    Blocked,
}

fn classify(inspect: &EmbedInspectResponse) -> TrafficClass {
    let action = inspect.action.to_ascii_lowercase();
    if inspect.blocked || action == "block" || action == "challenge" {
        TrafficClass::Blocked
    } else if action == "log" || action == "observe" {
        TrafficClass::Observed
    } else {
        TrafficClass::Clean
    }
}

pub async fn flush_traffic_batch(client: &reqwest::Client, api: &str, items: Vec<TrafficBatchItem>) {
    if items.is_empty() {
        return;
    }
    let url = format!("{}/api/waf/edge/traffic-batch", api.trim_end_matches('/'));
    let body = serde_json::json!({ "items": items });
    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!(count = body["items"].as_array().map(|a| a.len()).unwrap_or(0), "traffic batch flushed");
        }
        Ok(resp) => {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            tracing::warn!("traffic batch HTTP {status}: {detail}");
        }
        Err(e) => tracing::warn!("traffic batch flush: {e}"),
    }
}

pub fn spawn_traffic_flush_loop(
    api: String,
    client: reqwest::Client,
    counters: Arc<TrafficCounters>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let batch = counters.drain_batch();
            flush_traffic_batch(&client, &api, batch).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_three_tier() {
        let blocked = EmbedInspectResponse {
            allowed: false,
            blocked: true,
            action: "block".into(),
            rule_name: None,
            rule_id: None,
            category: None,
            severity: None,
            payload_sample: None,
            param_path: None,
            protection_stage: None,
            rule_set: None,
            plugin_name: None,
        };
        assert_eq!(classify(&blocked), TrafficClass::Blocked);

        let log = EmbedInspectResponse {
            action: "log".into(),
            blocked: false,
            allowed: true,
            ..blocked.clone()
        };
        assert_eq!(classify(&log), TrafficClass::Observed);

        let clean = EmbedInspectResponse {
            action: "allow".into(),
            blocked: false,
            allowed: true,
            ..blocked
        };
        assert_eq!(classify(&clean), TrafficClass::Clean);
    }
}
