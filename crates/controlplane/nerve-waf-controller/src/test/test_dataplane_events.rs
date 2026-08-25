//! Tests extracted from `dataplane/events/events.rs`.

use crate::dataplane::events::*;
use chrono::Utc;
use nerve_waf_sdk::WafEventRecord;

    fn sample(action: &str, severity: &str) -> WafEventRecord {
        WafEventRecord {
            id: "e1".into(),
            ts: Utc::now(),
            site_id: "waf-test".into(),
            layer: "l7".into(),
            action: action.into(),
            category: "xxe".into(),
            rule_id: Some("r1".into()),
            rule_name: Some("XXE".into()),
            severity: severity.into(),
            src_ip: Some("1.2.3.4".into()),
            dst_ip: None,
            dst_port: None,
            method: Some("POST".into()),
            host: Some("app".into()),
            uri: Some("/x".into()),
            user_agent: None,
            headers: None,
            body: None,
            payload_sample: None,
            matched_field: None,
            route_id: None,
            pod: None,
            message: None,
            trace_id: None,
            span_id: None,
            inspect_source: Some("waf-edge".into()),
            strategy_id: None,
            strategy_name: None,
            protection_stage: None,
            rule_set: None,
            plugin_name: None,
        }
    }

    #[test]
    fn edge_block_events_are_alert_candidates() {
        let events = vec![
            sample("block", "medium"),
            sample("log", "low"),
            sample("challenge", "medium"),
            sample("log", "high"),
        ];
        let ids: Vec<&str> = edge_alert_candidates(&events)
            .iter()
            .map(|e| e.action.as_str())
            .collect();
        assert_eq!(ids, vec!["block", "challenge", "log"]);
    }
