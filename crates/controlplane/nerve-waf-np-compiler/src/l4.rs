//! L4 policy compilation from K8s NetworkPolicy (Cilium-aligned semantics subset).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::lpm::{cidr_to_lpm_rules, LpmAllowRule};
use crate::selector::{labels_match_full, namespace_selector_matches, NamespaceSnapshot};
use crate::{K8sNetworkPolicySpec, NpPeerRule, NpPort};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PodSnapshot {
    pub namespace: String,
    pub name: String,
    pub ip: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L4Rule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dst_ip: Option<String>,
    #[serde(default)]
    pub dst_port: u16,
    #[serde(default = "default_proto")]
    pub proto: String,
    pub direction: String,
    pub action: String,
}

fn default_proto() -> String {
    "any".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EndpointPolicy {
    pub pod_ip: String,
    pub ingress_default_deny: bool,
    pub egress_default_deny: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CompiledL4Policy {
    pub version: u32,
    pub rules: Vec<L4Rule>,
    #[serde(default)]
    pub lpm_rules: Vec<LpmAllowRule>,
    pub endpoints: Vec<EndpointPolicy>,
    pub policies_applied: usize,
}

/// Compile K8s NetworkPolicy into L4 + LPM allow rules + per-endpoint default deny flags.
pub fn compile_k8s_np_l4(
    spec: &K8sNetworkPolicySpec,
    all_pods: &[PodSnapshot],
    all_namespaces: &[NamespaceSnapshot],
) -> CompiledL4Policy {
    compile_k8s_np_l4_inner(spec, all_pods, all_namespaces)
}

/// Backward-compatible wrapper without namespace snapshots.
pub fn compile_k8s_np_l4_pods(
    spec: &K8sNetworkPolicySpec,
    all_pods: &[PodSnapshot],
) -> CompiledL4Policy {
    compile_k8s_np_l4_inner(spec, all_pods, &[])
}

fn compile_k8s_np_l4_inner(
    spec: &K8sNetworkPolicySpec,
    all_pods: &[PodSnapshot],
    all_namespaces: &[NamespaceSnapshot],
) -> CompiledL4Policy {
    let mut out = CompiledL4Policy {
        version: 1,
        ..Default::default()
    };

    let selected: Vec<&PodSnapshot> = all_pods
        .iter()
        .filter(|p| {
            p.namespace == spec.namespace && labels_match_full(&spec.pod_selector, &p.labels)
        })
        .collect();

    if selected.is_empty() {
        return out;
    }

    let ingress_enforced = spec.policy_types.iter().any(|t| t == "Ingress");
    let egress_enforced = spec.policy_types.iter().any(|t| t == "Egress");

    for pod in &selected {
        out.endpoints.push(EndpointPolicy {
            pod_ip: pod.ip.clone(),
            ingress_default_deny: ingress_enforced,
            egress_default_deny: egress_enforced,
            namespace: Some(pod.namespace.clone()),
            pod_name: Some(pod.name.clone()),
        });

        if ingress_enforced {
            for ing in &spec.ingress {
                append_peer_rules(
                    &mut out.rules,
                    &mut out.lpm_rules,
                    ing,
                    all_pods,
                    all_namespaces,
                    &spec.namespace,
                    pod,
                    "ingress",
                );
            }
        }

        if egress_enforced {
            for eg in &spec.egress {
                append_peer_rules(
                    &mut out.rules,
                    &mut out.lpm_rules,
                    eg,
                    all_pods,
                    all_namespaces,
                    &spec.namespace,
                    pod,
                    "egress",
                );
            }
        }
    }

    out.rules = dedupe_rules(out.rules);
    out.policies_applied = 1;
    out
}

/// Merge multiple compiled policies (cluster-wide NP reconcile).
pub fn merge_l4_policies(policies: &[CompiledL4Policy]) -> CompiledL4Policy {
    let mut merged = CompiledL4Policy {
        version: 1,
        ..Default::default()
    };
    let mut ep_map: HashMap<String, EndpointPolicy> = HashMap::new();
    let mut rule_set: HashSet<String> = HashSet::new();

    for p in policies {
        merged.policies_applied += p.policies_applied;
        for ep in &p.endpoints {
            ep_map
                .entry(ep.pod_ip.clone())
                .and_modify(|e| {
                    e.ingress_default_deny |= ep.ingress_default_deny;
                    e.egress_default_deny |= ep.egress_default_deny;
                })
                .or_insert(ep.clone());
        }
        for r in &p.rules {
            let key = rule_key(r);
            if rule_set.insert(key) {
                merged.rules.push(r.clone());
            }
        }
        for lr in &p.lpm_rules {
            merged.lpm_rules.push(lr.clone());
        }
    }
    merged.endpoints = ep_map.into_values().collect();
    merged
}

// Peer rule expansion mirrors the NetworkPolicy peer schema field-by-field.
#[allow(clippy::too_many_arguments)]
fn append_peer_rules(
    rules: &mut Vec<L4Rule>,
    lpm_rules: &mut Vec<LpmAllowRule>,
    peer: &NpPeerRule,
    all_pods: &[PodSnapshot],
    all_namespaces: &[NamespaceSnapshot],
    np_namespace: &str,
    target: &PodSnapshot,
    direction: &str,
) {
    let ports = if peer.ports.is_empty() {
        vec![NpPort {
            port: 0,
            protocol: "any".into(),
        }]
    } else {
        peer.ports.clone()
    };

    if let Some(block) = &peer.ip_block {
        for port in &ports {
            let (src, dst) = if direction == "ingress" {
                (Some(block.cidr.clone()), Some(target.ip.clone()))
            } else {
                (Some(target.ip.clone()), Some(block.cidr.clone()))
            };
            rules.push(L4Rule {
                src_ip: src,
                dst_ip: dst,
                dst_port: port.port,
                proto: normalize_proto(&port.protocol),
                direction: direction.into(),
                action: "allow".into(),
            });
            lpm_rules.extend(cidr_to_lpm_rules(
                &block.cidr,
                direction,
                &target.ip,
                port.port,
                &normalize_proto(&port.protocol),
            ));
        }
        return;
    }

    let peer_pods: Vec<&PodSnapshot> = all_pods
        .iter()
        .filter(|p| peer_pod_matches(peer, p, np_namespace, all_namespaces))
        .collect();

    for pp in peer_pods {
        for port in &ports {
            rules.push(build_rule(
                direction,
                Some(pp.ip.clone()),
                Some(target.ip.clone()),
                target,
                port,
            ));
        }
    }
}

fn build_rule(
    direction: &str,
    src_ip: Option<String>,
    dst_ip: Option<String>,
    target: &PodSnapshot,
    port: &NpPort,
) -> L4Rule {
    let (src, dst) = if direction == "ingress" {
        (src_ip, dst_ip.or_else(|| Some(target.ip.clone())))
    } else {
        (src_ip.or_else(|| Some(target.ip.clone())), dst_ip)
    };
    L4Rule {
        src_ip: src,
        dst_ip: dst,
        dst_port: port.port,
        proto: normalize_proto(&port.protocol),
        direction: direction.into(),
        action: "allow".into(),
    }
}

fn normalize_proto(p: &str) -> String {
    match p.to_ascii_uppercase().as_str() {
        "TCP" => "tcp".into(),
        "UDP" => "udp".into(),
        _ => "any".into(),
    }
}

fn peer_pod_matches(
    peer: &NpPeerRule,
    pod: &PodSnapshot,
    np_namespace: &str,
    all_namespaces: &[NamespaceSnapshot],
) -> bool {
    if !peer
        .namespace_selector
        .as_object()
        .map(|o| o.is_empty())
        .unwrap_or(true)
    {
        if !namespace_selector_matches(&peer.namespace_selector, &pod.namespace, all_namespaces) {
            return false;
        }
    } else if peer
        .pod_selector
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false)
    {
        // podSelector-only peer in same NP namespace
        if pod.namespace != np_namespace {
            return false;
        }
    }
    if peer
        .pod_selector
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false)
    {
        return labels_match_full(&peer.pod_selector, &pod.labels);
    }
    pod.namespace == np_namespace
}

fn dedupe_rules(rules: Vec<L4Rule>) -> Vec<L4Rule> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in rules {
        let k = rule_key(&r);
        if seen.insert(k) {
            out.push(r);
        }
    }
    out
}

fn rule_key(r: &L4Rule) -> String {
    format!(
        "{:?}|{:?}|{}|{}|{}|{}",
        r.src_ip, r.dst_ip, r.dst_port, r.proto, r.direction, r.action
    )
}
