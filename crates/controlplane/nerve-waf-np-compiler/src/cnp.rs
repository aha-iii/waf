//! NerveClusterNetworkPolicy — first-party CNP (not a Cilium CRD copy) (P5).

use crate::l4::L4Rule;
use crate::selector::labels_match_full;
use crate::PodSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Nerve CNPolicy: app-level semantics and L7 path hints on top of K8s NP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NerveCNPolicySpec {
    pub id: String,
    pub namespace: String,
    pub pod_selector: serde_json::Value,
    #[serde(default)]
    pub ingress: Vec<CnpFlowRule>,
    #[serde(default)]
    pub egress: Vec<CnpFlowRule>,
    #[serde(default)]
    pub policy_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CnpFlowRule {
    #[serde(default)]
    pub to_apps: Vec<String>,
    #[serde(default)]
    pub to_cidrs: Vec<String>,
    #[serde(default)]
    pub ports: Vec<CnpPort>,
    #[serde(default)]
    pub http_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CnpPort {
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CompiledCnp {
    pub l4_rules: Vec<L4Rule>,
    pub l7_path_hints: Vec<String>,
}

pub fn compile_cnp(spec: &NerveCNPolicySpec, pods: &[PodSnapshot]) -> CompiledCnp {
    let mut out = CompiledCnp::default();
    let selected: Vec<&PodSnapshot> = pods
        .iter()
        .filter(|p| {
            p.namespace == spec.namespace && labels_match_full(&spec.pod_selector, &p.labels)
        })
        .collect();

    let ingress = spec.policy_types.is_empty() || spec.policy_types.iter().any(|t| t == "Ingress");
    let egress = spec.policy_types.is_empty() || spec.policy_types.iter().any(|t| t == "Egress");

    for pod in selected {
        if ingress {
            for rule in &spec.ingress {
                compile_cnp_flow(pod, rule, "ingress", pods, &mut out);
            }
        }
        if egress {
            for rule in &spec.egress {
                compile_cnp_flow(pod, rule, "egress", pods, &mut out);
            }
        }
    }
    out
}

fn compile_cnp_flow(
    pod: &PodSnapshot,
    rule: &CnpFlowRule,
    direction: &str,
    all_pods: &[PodSnapshot],
    out: &mut CompiledCnp,
) {
    for path in &rule.http_paths {
        out.l7_path_hints.push(format!(
            "{}/{} {} {} path={}",
            pod.namespace, pod.name, direction, rule.to_apps.join(","), path
        ));
    }
    for app in &rule.to_apps {
        for peer in all_pods {
            if peer.labels.get("app").map(|s| s.as_str()) == Some(app.as_str()) {
                for port in &rule.ports {
                    let (src, dst) = if direction == "ingress" {
                        (Some(peer.ip.clone()), Some(pod.ip.clone()))
                    } else {
                        (Some(pod.ip.clone()), Some(peer.ip.clone()))
                    };
                    out.l4_rules.push(L4Rule {
                        src_ip: src,
                        dst_ip: dst,
                        dst_port: port.port,
                        proto: normalize_proto(&port.protocol),
                        direction: direction.into(),
                        action: "allow".into(),
                    });
                }
            }
        }
    }
    for cidr in &rule.to_cidrs {
        for port in &rule.ports {
            let (src, dst) = if direction == "ingress" {
                (Some(cidr.clone()), Some(pod.ip.clone()))
            } else {
                (Some(pod.ip.clone()), Some(cidr.clone()))
            };
            out.l4_rules.push(L4Rule {
                src_ip: src,
                dst_ip: dst,
                dst_port: port.port,
                proto: normalize_proto(&port.protocol),
                direction: direction.into(),
                action: "allow".into(),
            });
        }
    }
}

fn normalize_proto(p: &str) -> String {
    match p.to_ascii_uppercase().as_str() {
        "TCP" => "tcp".into(),
        "UDP" => "udp".into(),
        _ => "any".into(),
    }
}
