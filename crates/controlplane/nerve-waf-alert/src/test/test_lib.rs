//! Tests extracted from `lib.rs`.

use crate::*;
use chrono::Utc;

    #[test]
    fn render_template_vars() {
        let payload = AlertPayload {
            ts: Utc::now(),
            severity: "high".into(),
            category: "sqli".into(),
            action: "block".into(),
            src_ip: Some("1.2.3.4".into()),
            host: Some("api.test".into()),
            uri: Some("/x".into()),
            message: "test".into(),
            site_id: Some("default".into()),
            trace_id: Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890".into()),
            span_id: Some("9f8e7d6c-5b4a-3210-fedc-ba9876543210".into()),
        };
        let s = render_template("级别${severity_label} trace=${trace_id}", &payload);
        assert!(s.contains("高危"));
        assert!(s.contains("a1b2c3d4"));
    }

    #[test]
    fn migrate_wecom_from_legacy_webhooks() {
        let cfg = normalize_config(AlertConfig {
            enabled: true,
            webhooks: vec!["https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=abc".into()],
            ..Default::default()
        });
        assert!(cfg.wecom.webhook_url.contains("qyapi.weixin.qq.com"));
        assert!(cfg.webhooks.is_empty());
    }
