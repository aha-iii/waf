use nerve_waf_dataplane::WafEdgeSnapshot;
use serde_json::Value;

use super::list::parse_snapshot_response;

pub async fn watch_dataplane(
    api_base: &str,
    since: u64,
    timeout_sec: u64,
) -> Result<Option<WafEdgeSnapshot>, String> {
    let url = format!(
        "{}/api/waf/edge/watch?since={since}&timeout_sec={}",
        api_base.trim_end_matches('/'),
        timeout_sec.clamp(1, 120)
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("watch dataplane: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("watch HTTP {}", resp.status()));
    }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    if v.get("unchanged").and_then(|x| x.as_bool()) == Some(true) {
        return Ok(None);
    }
    parse_snapshot_response(v).map(Some)
}

pub async fn ack_generation(api_base: &str, generation: u64) {
    let url = format!("{}/api/waf/edge/ack", api_base.trim_end_matches('/'));
    let body = serde_json::json!({ "generation": generation });
    match reqwest::Client::new().post(&url).json(&body).send().await {
        Ok(r) if r.status().is_success() => {
            tracing::debug!(generation, "edge ack sent");
        }
        Ok(r) => tracing::warn!("edge ack HTTP {}", r.status()),
        Err(e) => tracing::warn!("edge ack: {e}"),
    }
}
