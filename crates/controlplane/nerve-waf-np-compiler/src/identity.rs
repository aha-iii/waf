//! Pod security identity — stable ID independent of Pod IP (P2).

use crate::PodSnapshot;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct IdentityRecord {
    pub security_id: u32,
    pub namespace: String,
    pub name: String,
    pub pod_ip: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct IdentityMap {
    pub version: u32,
    pub records: Vec<IdentityRecord>,
}

/// Deterministic security_id from namespace/name (FNV-1a 32-bit).
pub fn security_id_for(ns: &str, name: &str) -> u32 {
    let key = format!("{ns}/{name}");
    let mut hash: u32 = 0x811c_9dc5;
    for b in key.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash | 1 // avoid 0
}

pub fn build_identity_map(pods: &[PodSnapshot]) -> IdentityMap {
    IdentityMap {
        version: 1,
        records: pods
            .iter()
            .map(|p| IdentityRecord {
                security_id: security_id_for(&p.namespace, &p.name),
                namespace: p.namespace.clone(),
                name: p.name.clone(),
                pod_ip: p.ip.clone(),
                labels: p.labels.clone(),
            })
            .collect(),
    }
}

/// Rewrite L4 rules to include security_id fields when endpoint matches.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct IdentityL4Rule {
    pub src_security_id: Option<u32>,
    pub dst_security_id: Option<u32>,
    pub src_ip: Option<String>,
    pub dst_ip: Option<String>,
    pub dst_port: u16,
    pub proto: String,
    pub direction: String,
    pub action: String,
}

pub fn attach_identity_to_rules(
    id_map: &IdentityMap,
    rules: &[crate::l4::L4Rule],
) -> Vec<IdentityL4Rule> {
    let ip_to_id: HashMap<&str, u32> = id_map
        .records
        .iter()
        .map(|r| (r.pod_ip.as_str(), r.security_id))
        .collect();
    rules
        .iter()
        .map(|r| IdentityL4Rule {
            src_security_id: r.src_ip.as_deref().and_then(|ip| ip_to_id.get(ip).copied()),
            dst_security_id: r.dst_ip.as_deref().and_then(|ip| ip_to_id.get(ip).copied()),
            src_ip: r.src_ip.clone(),
            dst_ip: r.dst_ip.clone(),
            dst_port: r.dst_port,
            proto: r.proto.clone(),
            direction: r.direction.clone(),
            action: r.action.clone(),
        })
        .collect()
}
