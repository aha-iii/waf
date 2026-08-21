//! FQDN egress policy — DNS-cache driven (P5, first-party, not Cilium FQDN copy).

use crate::l4::L4Rule;
use crate::PodSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FqdnPolicySpec {
    pub id: String,
    pub namespace: String,
    pub pod_selector: serde_json::Value,
    #[serde(default)]
    pub allow_fqdns: Vec<String>,
    #[serde(default)]
    pub ports: Vec<FqdnPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FqdnPort {
    pub port: u16,
    pub protocol: String,
}

/// DNS resolution snapshot (written by Nerve dns-watcher).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DnsCacheSnapshot {
    pub entries: Vec<DnsCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DnsCacheEntry {
    pub fqdn: String,
    pub ips: Vec<String>,
    pub ttl_sec: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CompiledFqdn {
    pub fqdn_allow: Vec<FqdnAllowEntry>,
    pub l4_rules: Vec<L4Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FqdnAllowEntry {
    pub fqdn: String,
    pub resolved_ips: Vec<String>,
    pub port: u16,
    pub proto: String,
}

pub fn compile_fqdn_policy(
    spec: &FqdnPolicySpec,
    pods: &[PodSnapshot],
    dns: &DnsCacheSnapshot,
) -> CompiledFqdn {
    use crate::selector::labels_match_full;
    let mut out = CompiledFqdn::default();
    let selected: Vec<&PodSnapshot> = pods
        .iter()
        .filter(|p| {
            p.namespace == spec.namespace && labels_match_full(&spec.pod_selector, &p.labels)
        })
        .collect();

    for pod in selected {
        for fqdn in &spec.allow_fqdns {
            let ips = dns
                .entries
                .iter()
                .find(|e| e.fqdn == *fqdn)
                .map(|e| e.ips.clone())
                .unwrap_or_default();
            for port in &spec.ports {
                out.fqdn_allow.push(FqdnAllowEntry {
                    fqdn: fqdn.clone(),
                    resolved_ips: ips.clone(),
                    port: port.port,
                    proto: normalize_proto(&port.protocol),
                });
                for ip in &ips {
                    out.l4_rules.push(L4Rule {
                        src_ip: Some(pod.ip.clone()),
                        dst_ip: Some(ip.clone()),
                        dst_port: port.port,
                        proto: normalize_proto(&port.protocol),
                        direction: "egress".into(),
                        action: "allow".into(),
                    });
                }
            }
        }
    }
    out
}

fn normalize_proto(p: &str) -> String {
    match p.to_ascii_uppercase().as_str() {
        "TCP" => "tcp".into(),
        "UDP" => "udp".into(),
        _ => "any".into(),
    }
}
