//! Tests extracted from `controlplane/ai_policy_sim/ai_policy_sim.rs`.

use crate::controlplane::ai_policy_sim::*;

#[test]
fn rejects_empty_change_set() {
    let body = SimulateBody {
        site_id: Some("default".into()),
        disable_rules: vec![],
        whitelist_uri_prefixes: vec![],
        sample_limit: 100,
    };
    assert!(validate_simulate_body(&body).is_some());
}

#[test]
fn rejects_prefix_without_slash() {
    let body = SimulateBody {
        site_id: Some("default".into()),
        disable_rules: vec![],
        whitelist_uri_prefixes: vec!["api/health".into()],
        sample_limit: 100,
    };
    let err = validate_simulate_body(&body).unwrap();
    assert_eq!(err["error"], "invalid_prefix");
}

#[test]
fn risk_insufficient_when_eo_blocks() {
    assert_eq!(classify_risk(0.0, 0, 50), "insufficient_data");
}
