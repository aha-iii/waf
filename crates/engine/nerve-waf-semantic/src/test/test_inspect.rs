//! Tests extracted from `inspect/inspect.rs`.

use crate::inspect::*;
use crate::SemanticConfig;
use nerve_waf_l7::InspectRequest;
use nerve_waf_libinjection::LibInjectionConfig;
use nerve_waf_sdk::ProtectionModules;

#[test]
fn detects_double_encoded_sqli() {
    let cfg = SemanticConfig::default();
    let li = LibInjectionConfig::default();
    let modules = ProtectionModules::default();
    let hit = inspect_semantic(
        &cfg,
        &li,
        &modules,
        &InspectRequest {
            uri: "/?id=%2527%2520OR%25201%253D1".into(),
            ..Default::default()
        },
        None,
    );
    assert!(
        hit.is_some(),
        "expected semantic hit on double-encoded sqli"
    );
}

#[test]
fn path_prefix_whitelist_skips() {
    let mut cfg = SemanticConfig::default();
    cfg.whitelist.path_prefixes = vec!["/static".into()];
    let li = LibInjectionConfig::default();
    let modules = ProtectionModules::default();
    let hit = inspect_semantic(
        &cfg,
        &li,
        &modules,
        &InspectRequest {
            uri: "/static/?id=%2527%2520OR%25201%253D1".into(),
            ..Default::default()
        },
        None,
    );
    assert!(hit.is_none());
}

#[test]
fn disabled_xss_rule_941100_is_skipped() {
    let cfg = SemanticConfig::default();
    let li = LibInjectionConfig::default();
    let mut modules = ProtectionModules::default();
    let xss = modules.modules.entry("xss".into()).or_default();
    xss.enabled = true;
    xss.action = "block".into();
    xss.disabled_rules = vec!["941100".into()];
    let hit = inspect_semantic(
        &cfg,
        &li,
        &modules,
        &InspectRequest {
            uri: "/x?q=<script>alert(1)</script>".into(),
            ..Default::default()
        },
        None,
    );
    assert!(
        hit.is_none(),
        "disabled 941100 must not fire on query XSS (and 941101 must not scan query), got {hit:?}"
    );
}
