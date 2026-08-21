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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn prefers_json_when_accept_json_only() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        assert!(prefers_json(&h));
    }

    #[test]
    fn prefers_html_for_browser_accept() {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        assert!(!prefers_json(&h));
    }

    #[test]
    fn prefers_html_when_accept_missing() {
        assert!(!prefers_json(&HeaderMap::new()));
    }

    #[test]
    fn render_html_fills_placeholders() {
        let html = render_html(&BlockPageFields {
            event_id: "evt_abc".into(),
            action: "challenge".into(),
            rule_id: "open-redirect-param".into(),
            site_id: "app-es4t0y50".into(),
        });
        assert!(html.contains("evt_abc"));
        assert!(html.contains("open-redirect-param"));
        assert!(html.contains("app-es4t0y50"));
        assert!(!html.contains("__NERVE_"));
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("事件 ID"));
        assert!(html.contains("技术详情"));
        assert!(html.contains("#1677ff") || html.contains("1677ff"));
    }

    #[test]
    fn render_html_escapes_xss() {
        let html = render_html(&BlockPageFields {
            event_id: "e".into(),
            action: "block".into(),
            rule_id: "<script>".into(),
            site_id: "s".into(),
        });
        assert!(html.contains("id=\"rule\">&lt;script&gt;</dd>"));
        assert!(!html.contains("id=\"rule\"><script>"));
    }

    #[test]
    fn render_json_includes_page_parity_fields() {
        let raw = render_json(
            &BlockPageFields {
                event_id: "evt_1".into(),
                action: "block".into(),
                rule_id: "r1".into(),
                site_id: "s1".into(),
            },
            Some("demo-rule"),
        );
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["blocked"], true);
        assert_eq!(v["status"], 403);
        assert_eq!(v["product"], "waf");
        assert_eq!(v["layer"], "l7");
        assert_eq!(v["reason"], "rule_hit");
        assert_eq!(v["retry_after_sec"], 0);
        assert_eq!(v["event_id"], "evt_1");
        assert_eq!(v["action"], "block");
        assert_eq!(v["rule_id"], "r1");
        assert_eq!(v["rule_name"], "demo-rule");
        assert_eq!(v["site_id"], "s1");
        assert_eq!(v["message"], "Blocked by Nerve WAF");
    }
}
