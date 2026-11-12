//! Tests extracted from `dataplane/traffic/traffic.rs`.

use crate::dataplane::traffic::*;
use nerve_waf_embed::EmbedInspectResponse;

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
        timing: None,
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
