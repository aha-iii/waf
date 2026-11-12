use nerve_waf_dataplane::WafEdgeSnapshot;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
// Envelope is fully deserialized; only `dataplane` is consumed.
#[allow(dead_code)]
struct DataplaneEnvelope {
    ok: Option<bool>,
    generation: Option<u64>,
    #[serde(default)]
    unchanged: bool,
    dataplane: Option<WafEdgeSnapshot>,
}

pub async fn fetch_dataplane(api_base: &str) -> Result<WafEdgeSnapshot, String> {
    let url = format!("{}/api/waf/edge/dataplane", api_base.trim_end_matches('/'));
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("fetch dataplane: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("dataplane HTTP {}", resp.status()));
    }
    parse_snapshot_response(resp.json().await.map_err(|e| e.to_string())?)
}

pub fn parse_snapshot_response(v: Value) -> Result<WafEdgeSnapshot, String> {
    if v.get("ok").and_then(|x| x.as_bool()) == Some(false) {
        return Err(v
            .get("error")
            .and_then(|x| x.as_str())
            .unwrap_or("dataplane error")
            .into());
    }
    if let Ok(env) = serde_json::from_value::<DataplaneEnvelope>(v.clone()) {
        if let Some(snap) = env.dataplane {
            return Ok(snap);
        }
    }
    serde_json::from_value(v.get("dataplane").cloned().unwrap_or(v))
        .map_err(|e| format!("parse dataplane: {e}"))
}
