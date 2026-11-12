//! Tests extracted from `dataplane/proxy/block_page/block_page.rs`.

use crate::dataplane::proxy::block_page::*;
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
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
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
