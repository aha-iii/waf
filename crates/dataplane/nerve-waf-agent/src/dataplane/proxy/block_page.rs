//! Edge block page: HTML for browsers, JSON for API clients (content negotiation).

use axum::http::HeaderMap;

const TEMPLATE: &str = include_str!("../../../assets/block-page.html");

#[derive(Debug, Clone)]
pub struct BlockPageFields {
    pub event_id: String,
    pub action: String,
    pub rule_id: String,
    pub site_id: String,
}

/// `Accept` has `application/json` and not `text/html` → JSON; else HTML.
pub fn prefers_json(headers: &HeaderMap) -> bool {
    let Some(raw) = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let lower = raw.to_ascii_lowercase();
    lower.contains("application/json") && !lower.contains("text/html")
}

pub fn new_event_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("evt_{:x}", (nanos as u64).wrapping_mul(0x9e37_79b9) ^ (nanos >> 32) as u64)
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn render_html(fields: &BlockPageFields) -> String {
    TEMPLATE
        .replace("__NERVE_EVENT__", &html_escape(&fields.event_id))
        .replace("__NERVE_ACTION__", &html_escape(&fields.action))
        .replace("__NERVE_RULE__", &html_escape(&fields.rule_id))
        .replace("__NERVE_SITE__", &html_escape(&fields.site_id))
}

pub fn render_json(fields: &BlockPageFields, rule_name: Option<&str>) -> String {
    // Shared fields aligned with Nerve DDoS edge block JSON.
    serde_json::json!({
        "blocked": true,
        "action": fields.action,
        "product": "waf",
        "layer": "l7",
        "reason": "rule_hit",
        "message": "Blocked by Nerve WAF",
        "event_id": fields.event_id,
        "retry_after_sec": 0,
        "score": 0,
        "rule_id": fields.rule_id,
        "rule_name": rule_name,
        "site_id": fields.site_id,
        "status": 403,
    })
    .to_string()
}
