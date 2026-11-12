//! LPM trie rule generation for ipBlock CIDR (P1).

use crate::l4::L4Rule;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LpmAllowRule {
    pub prefix: String,
    pub direction: String,
    pub endpoint_ip: String,
    pub peer_role: String, // src or dst
    pub dst_port: u16,
    pub proto: String,
    pub action: String,
}

/// Expand ipBlock CIDR into LPM-friendly allow entries (direction-aware).
pub fn cidr_to_lpm_rules(
    cidr: &str,
    direction: &str,
    endpoint_ip: &str,
    port: u16,
    proto: &str,
) -> Vec<LpmAllowRule> {
    vec![LpmAllowRule {
        prefix: cidr.to_string(),
        direction: direction.into(),
        endpoint_ip: endpoint_ip.into(),
        peer_role: if direction == "ingress" {
            "src".into()
        } else {
            "dst".into()
        },
        dst_port: port,
        proto: proto.into(),
        action: "allow".into(),
    }]
}

/// Parse IPv4 CIDR to prefix length + network address string.
// Reserved for the LPM compilation path (NetworkPolicy allow rules).
#[allow(dead_code)]
pub fn parse_v4_prefix(cidr: &str) -> Option<(u8, Ipv4Addr)> {
    let (ip, plen) = cidr.split_once('/')?;
    let addr: Ipv4Addr = ip.parse().ok()?;
    let plen: u8 = plen.parse().ok()?;
    if plen > 32 {
        return None;
    }
    Some((plen, addr))
}

/// Merge LPM rules into L4 hash rules for /32 endpoints; non-/32 stay in LPM list.
// Reserved for the LPM compilation path (NetworkPolicy allow rules).
#[allow(dead_code)]
pub fn supplement_l4_with_lpm(l4: &mut Vec<L4Rule>, lpm: &[LpmAllowRule]) {
    for r in lpm {
        if r.prefix.ends_with("/32") {
            let ip = r.prefix.trim_end_matches("/32");
            if r.direction == "ingress" {
                l4.push(L4Rule {
                    src_ip: Some(ip.into()),
                    dst_ip: Some(r.endpoint_ip.clone()),
                    dst_port: r.dst_port,
                    proto: r.proto.clone(),
                    direction: r.direction.clone(),
                    action: r.action.clone(),
                });
            } else {
                l4.push(L4Rule {
                    src_ip: Some(r.endpoint_ip.clone()),
                    dst_ip: Some(ip.into()),
                    dst_port: r.dst_port,
                    proto: r.proto.clone(),
                    direction: r.direction.clone(),
                    action: r.action.clone(),
                });
            }
        }
    }
}
