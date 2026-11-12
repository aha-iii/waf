//! Tests extracted from `controlplane/ai_incident/ai_incident.rs`.

use crate::controlplane::ai_incident::*;

#[test]
fn dedupes_actions() {
    let out = dedupe_preserve_order(vec!["a".into(), "b".into(), "a".into()]);
    assert_eq!(out, vec!["a", "b"]);
}

#[test]
fn saeitizes_incident_suffix() {
    assert_eq!(saeitize_id_suffix("evt-abc/123"), "evt-abc123");
}
