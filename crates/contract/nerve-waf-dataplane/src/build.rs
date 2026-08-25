use crate::snapshot::{EdgeRoute, EdgeSiteMeta, WafEdgeSnapshot};
use crate::snapshot_checksum;
use chrono::Utc;
use nerve_waf_sdk::L7Upstream;
use std::collections::HashMap;

pub use crate::snapshot::{GlobalInspectOverlay, SiteInspectSnapshot};

/// Control-plane loaded data; filled by `nerve-waf-controller` from AppState.
#[derive(Debug, Clone, Default)]
pub struct WafEdgeBuildInput {
    pub generation: u64,
    pub routes: Vec<EdgeRoute>,
    pub upstreams: Vec<L7Upstream>,
    pub sites: Vec<EdgeSiteMeta>,
    pub site_inspect: HashMap<String, SiteInspectSnapshot>,
    pub global: GlobalInspectOverlay,
}

pub fn build_waf_edge_snapshot(input: WafEdgeBuildInput) -> WafEdgeSnapshot {
    let mut snap = WafEdgeSnapshot {
        generation: input.generation,
        checksum: String::new(),
        built_at: Utc::now().to_rfc3339(),
        routes: input.routes,
        upstreams: input.upstreams,
        sites: input.sites,
        site_inspect: input.site_inspect,
        global: input.global,
    };
    snap.checksum = snapshot_checksum(&snap);
    snap
}
