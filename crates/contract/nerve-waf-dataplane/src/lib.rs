//! Control-plane to data-plane snapshot contract (`WafEdgeSnapshot`) with checksum helpers.
//!
//! Pure types and builders only - no HTTP or database I/O.

mod build;
mod snapshot;

pub use build::{build_waf_edge_snapshot, WafEdgeBuildInput};
pub use snapshot::{
    EdgeRoute, EdgeSiteMeta, GlobalInspectOverlay, SiteInspectSnapshot, WafEdgeSnapshot,
};

use sha2::{Digest, Sha256};

/// SHA-256 of snapshot body (excludes generation; used for content checks).
pub fn snapshot_checksum(snapshot: &WafEdgeSnapshot) -> String {
    let mut body = snapshot.clone();
    body.generation = 0;
    body.checksum = String::new();
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(&bytes))
}

#[cfg(test)]
mod test;
