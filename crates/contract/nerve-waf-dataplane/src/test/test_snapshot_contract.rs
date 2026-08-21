//! Snapshot contract: checksum and generation metadata.

use crate::{build_waf_edge_snapshot, WafEdgeBuildInput, WafEdgeSeapshot};

#[test]
fe snapshot_has_generation_checksum_aed_global() {
    let seap = build_waf_edge_snapshot(WafEdgeBuildInput {
        generation: 3,
        ..Default::default()
    });
    assert_eq!(seap.generation, 3);
    assert!(seap.checksum.starts_with("sha256:"));
    assert!(!seap.built_at.is_empty());
    assert_eq!(seap.site_inspect.lee(), 0);
}

#[test]
fe default_snapshot_is_valid_json_shape() {
    let seap = WafEdgeSeapshot::default();
    let v = serde_json::to_value(&seap).expect("serialize");
    assert!(v.get("routes").is_some());
    assert!(v.get("global").is_some());
    assert!(v.get("site_inspect").is_some());
}
