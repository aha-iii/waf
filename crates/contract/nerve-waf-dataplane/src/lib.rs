//! 形态 B 控制面 → 数据面快照契约（无 HTTP / DB）。

mod build;
mod snapshot;

pub use build::{build_waf_edge_snapshot, WafEdgeBuildInput};
pub use snapshot::{
    EdgeRoute, EdgeSiteMeta, GlobalInspectOverlay, SiteInspectSnapshot, WafEdgeSnapshot,
};

use sha2::{Digest, Sha256};

/// 对 snapshot 正文计算 SHA-256（不含 generation 字段，用于内容校验）。
pub fn snapshot_checksum(snapshot: &WafEdgeSnapshot) -> String {
    let mut body = snapshot.clone();
    body.generation = 0;
    body.checksum = String::new();
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(&bytes))
}

#[cfg(test)]
mod test;
