//! Tests extracted from `lib.rs`.

use crate::*;
    use chrono::Utc;

    fn sample_event(category: &str, action: &str, uri: &str, payload: &str) -> WafEventRecord {
        WafEventRecord {
            id: "t1".into(),
            ts: Utc::now(),
            site_id: "default".into(),
            layer: "l7".into(),
            action: action.into(),
            category: category.into(),
            rule_id: Some("942100".into()),
            rule_name: Some("SQLi".into()),
            severity: "high".into(),
            src_ip: Some("1.2.3.4".into()),
            dst_ip: None,
            dst_port: None,
            method: Some("GET".into()),
            host: Some("app".into()),
            uri: Some(uri.into()),
            user_agent: Some("Mozilla/5.0".into()),
            headers: None,
            body: None,
            payload_sample: Some(payload.into()),
            matched_field: Some("uri".into()),
            route_id: None,
            pod: None,
            message: None,
            trace_id: None,
            span_id: None,
            inspect_source: None,
            strategy_id: None,
            strategy_name: None,
            protection_stage: None,
            rule_set: None,
            plugin_name: None,
        }
    }

    #[test]
    fn confirms_obvious_sqli() {
        let ev = sample_event(
            "sqli",
            "block",
            "/search?q=1+union+select+null",
            "union select",
        );
        let r = analyze_event(&ev, &TriageContext::default());
        assert!(matches!(
            r.verdict,
            TriageVerdict::ConfirmedAttack | TriageVerdict::LikelyAttack
        ));
        assert!(r.risk_score >= 40);
    }

    #[test]
    fn flags_static_asset_fp() {
        let ev = sample_event("sqli", "log", "/assets/app.js?v=1", "select");
        let r = analyze_event(
            &ev,
            &TriageContext {
                global_mode: "detect".into(),
                ..Default::default()
            },
        );
        assert!(matches!(
            r.verdict,
            TriageVerdict::LikelyFalsePositive | TriageVerdict::NeedsReview
        ));
    }
