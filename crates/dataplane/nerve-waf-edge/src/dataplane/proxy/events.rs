use axum::http::HeaderMap;
use chrono::Utc;
use nerve_waf_embed::EmbedInspectResponse;

/// 与 API 侧 MAX_HEADERS_LEN / MAX_BODY_LEN 对齐
const EDGE_MAX_HEADERS_LEN: usize = 4096;
const EDGE_MAX_BODY_LEN: usize = 8192;

/// 将 HeaderMap 转为换行分隔的 `Name: Value` 字符串（与前端 formatHeaders 期望一致）。
fn headermap_to_string(h: &HeaderMap) -> String {
    let mut lines = Vec::new();
    for (k, v) in h.iter() {
        if let Ok(s) = v.to_str() {
            lines.push(format!("{}: {}", k.as_str(), s));
        }
    }
    lines.join("\n")
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut end = max_len;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    let mut t = s[..end].to_string();
    t.push_str("...(truncated)");
    t
}

pub async fn report_block_event(
    client: &reqwest::Client,
    api: &str,
    site_id: &str,
    method: &str,
    host: &str,
    uri: &str,
    src_ip: &str,
    user_agent: &str,
    event_id: &str,
    inspect: EmbedInspectResponse,
    raw_headers: &HeaderMap,
    raw_body: &str,
) {
    let url = format!("{}/api/waf/edge/events", api.trim_end_matches('/'));
    let headers_str = truncate(&headermap_to_string(raw_headers), EDGE_MAX_HEADERS_LEN);
    let body_str = truncate(raw_body, EDGE_MAX_BODY_LEN);
    let body = serde_json::json!({
        "events": [{
            "id": event_id,
            "ts": Utc::now().to_rfc3339(),
            "site_id": site_id,
            "layer": "l7",
            "action": inspect.action,
            "category": inspect.category.clone().unwrap_or_else(|| "attack".into()),
            "rule_id": inspect.rule_id,
            "rule_name": inspect.rule_name,
            "severity": inspect.severity.clone().unwrap_or_else(|| "medium".into()),
            "src_ip": src_ip,
            "method": method,
            "host": host,
            "uri": uri,
            "user_agent": user_agent,
            "payload_sample": inspect.payload_sample,
            "param_path": inspect.param_path,
            "inspect_source": "waf-edge",
            "protection_stage": inspect.protection_stage,
            "rule_set": inspect.rule_set,
            "plugin_name": inspect.plugin_name,
            "headers": headers_str,
            "body": body_str,
        }]
    });
    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(
                site_id = %site_id,
                rule_id = ?inspect.rule_id,
                "edge event ingested"
            );
        }
        Ok(resp) => {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            tracing::warn!("edge event ingest HTTP {status}: {detail}");
        }
        Err(e) => tracing::warn!("edge event ingest: {e}"),
    }
}
