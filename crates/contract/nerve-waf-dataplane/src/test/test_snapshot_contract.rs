//! Snapshot contract: checksum and generation metadata.

use crate::{build_waf_edge_snapshot, WafEdgeBuildInput, WafEdgeSnapshot};

#[test]
fn snapshot_has_generation_checksum_and_global() {
    let snap = build_waf_edge_snapshot(WafEdgeBuildInput {
        generation: 3,
        ..Default::default()
    });
    assert_eq!(snap.generation, 3);
    assert!(snap.checksum.starts_with("sha256:"));
    assert!(!snap.built_at.is_empty());
    assert_eq!(snap.site_inspect.len(), 0);
}

#[test]
fn default_snapshot_is_valid_json_shape() {
    let snap = WafEdgeSnapshot::default();
    let v = serde_json::to_value(&snap).expect("serialize");
    assert!(v.get("routes").is_some());
    assert!(v.get("global").is_some());
    assert!(v.get("site_inspect").is_some());
}
