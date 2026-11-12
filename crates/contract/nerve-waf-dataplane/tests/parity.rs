//! Edge dataplane vs embed inspect verdict parity (fixture-level).
//!
//! Integration test: links `nerve_waf_dataplane` as a normal dependency so the
//! snapshot types unify with the ones `nerve_waf_embed` links (the crate cannot
//! be a dev-dependency of a unit test without duplicating the lib under cfg(test)).

use nerve_waf_dataplane::{
    build_waf_edge_snapshot, GlobalInspectOverlay, SiteInspectSnapshot, WafEdgeBuildInput,
};
use nerve_waf_embed::{EmbedInspectRequest, EmbeddedWaf};
use nerve_waf_sdk::ProtectionModules;
use nerve_waf_semantic::SemanticConfig;
use std::collections::HashMap;

#[test]
fn snapshot_roundtrip_preserves_site_inspect() {
    let mut site_inspect = HashMap::new();
    site_inspect.insert(
        "default".into(),
        SiteInspectSnapshot {
            modules: ProtectionModules::default(),
            semantic: SemanticConfig::default(),
            ..Default::default()
        },
    );
    let input = WafEdgeBuildInput {
        generation: 7,
        routes: vec![],
        upstreams: vec![],
        sites: vec![],
        site_inspect,
        global: GlobalInspectOverlay::default(),
    };
    let snap = build_waf_edge_snapshot(input);
    assert_eq!(snap.generation, 7);
    assert!(snap.checksum.starts_with("sha256:"));
    assert!(snap.site_inspect.contains_key("default"));
}

#[test]
fn embed_inspect_with_snapshot_matches_policy_path() {
    let waf = EmbeddedWaf::new(None);
    let site = SiteInspectSnapshot {
        modules: ProtectionModules::default(),
        semantic: SemanticConfig::default(),
        ..Default::default()
    };
    let global = GlobalInspectOverlay::default();
    let req = EmbedInspectRequest {
        method: "GET".into(),
        host: "example.com".into(),
        uri: "/".into(),
        headers: "{}".into(),
        body: String::new(),
        src_ip: "10.0.0.1".into(),
        user_agent: "parity-test".into(),
        response_body: None,
    };
    let a = waf.inspect_with_snapshot(req.clone(), &site, &global);
    let b = waf.inspect_with_policy(
        req,
        Some(&nerve_waf_embed::EmbedInspectPolicy {
            modules: site.modules,
            semantic: site.semantic,
        }),
    );
    assert_eq!(a.action, b.action);
    assert_eq!(a.blocked, b.blocked);
}
