//! Tests extracted from `selector/selector.rs`.

use crate::selector::*;
use std::collections::HashMap;

#[test]
fn namespace_selector_by_label() {
    let nss = vec![NamespaceSnapshot {
        name: "backend".into(),
        labels: HashMap::from([("team".into(), "backend".into())]),
    }];
    let sel = serde_json::json!({"matchLabels": {"team": "backend"}});
    assert!(namespace_selector_matches(&sel, "backend", &nss));
    assert!(!namespace_selector_matches(&sel, "default", &nss));
}
