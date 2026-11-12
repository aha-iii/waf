//! Tests extracted from `sensitivity/sensitivity.rs`.

use crate::sensitivity::*;

#[test]
fn low_sensitivity_high_severity_pl1_only() {
    assert!(rule_matches_sensitivity(1, "critical", "low"));
    assert!(!rule_matches_sensitivity(2, "critical", "low"));
    assert!(!rule_matches_sensitivity(1, "medium", "low"));
}

#[test]
fn high_sensitivity_includes_all() {
    assert!(rule_matches_sensitivity(4, "low", "high"));
    assert!(rule_matches_sensitivity(2, "medium", "high"));
}

#[test]
fn medium_cumulative() {
    assert!(rule_matches_sensitivity(2, "medium", "medium"));
    assert!(!rule_matches_sensitivity(3, "medium", "medium"));
    assert!(!rule_matches_sensitivity(2, "low", "medium"));
}
