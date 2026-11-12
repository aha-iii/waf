use nerve_waf_dataplane::{SiteInspectSnapshot, WafEdgeSnapshot};
use nerve_waf_sdk::L7Upstream;
use std::collections::HashMap;

pub use nerve_waf_dataplane::{EdgeRoute, GlobalInspectOverlay};

#[derive(Debug, Clone, Default)]
pub struct EdgeRuntime {
    pub snap: WafEdgeSnapshot,
    host_index: HashMap<String, EdgeRoute>,
    upstreams: HashMap<String, L7Upstream>,
}

impl EdgeRuntime {
    pub fn from_snapshot(snap: WafEdgeSnapshot) -> Self {
        let mut host_index = HashMap::new();
        for r in &snap.routes {
            host_index.insert(r.host.trim().to_lowercase(), r.clone());
        }
        let mut upstreams = HashMap::new();
        for u in &snap.upstreams {
            upstreams.insert(u.id.clone(), u.clone());
        }
        Self {
            snap,
            host_index,
            upstreams,
        }
    }

    pub fn generation(&self) -> u64 {
        self.snap.generation
    }

    pub fn site_inspect(&self, site_id: &str) -> Option<&SiteInspectSnapshot> {
        self.snap.site_inspect.get(site_id)
    }

    pub fn global(&self) -> &GlobalInspectOverlay {
        &self.snap.global
    }

    pub fn match_route(&self, host: &str) -> Option<&EdgeRoute> {
        let h = host.trim().to_lowercase();
        if let Some(r) = self.host_index.get(&h) {
            return Some(r);
        }
        self.host_index.get("*")
    }

    pub fn upstream(&self, upstream_id: &str) -> Option<&L7Upstream> {
        self.upstreams.get(upstream_id)
    }
}

pub type SharedRuntime = std::sync::Arc<tokio::sync::RwLock<EdgeRuntime>>;
