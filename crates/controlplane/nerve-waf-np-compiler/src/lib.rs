//! Compile NetworkPolicy-like configs into SyncPlan / L4 hints.

mod cnp;
mod fqdn;
mod identity;
mod l4;
mod lpm;
mod selector;

use nerve_waf_sdk::{LogLevel, PluginAction, SyncPlan};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub use cnp::{compile_cnp, CompiledCnp, NerveCNPolicySpec};
pub use fqdn::{compile_fqdn_policy, CompiledFqdn, DnsCacheSnapshot, FqdnPolicySpec};
pub use identity::{
    attach_identity_to_rules, build_identity_map, IdentityL4Rule, IdentityMap, IdentityRecord,
};
pub use l4::{
    compile_k8s_np_l4, compile_k8s_np_l4_pods, merge_l4_policies, CompiledL4Policy, EndpointPolicy,
    L4Rule, PodSnapshot,
};
pub use lpm::{cidr_to_lpm_rules, LpmAllowRule};
pub use selector::{labels_match_full, namespace_selector_matches, NamespaceSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpRule {
    pub id: String,
    pub namespace: String,
    pub pod_selector: String,
    pub allow_cidrs: Vec<String>,
    pub allow_ports: Vec<u16>,
    #[serde(default)]
    pub deny_all_egress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sNetworkPolicySpec {
    pub id: String,
    pub namespace: String,
    pub pod_selector: serde_json::Value,
    pub policy_types: Vec<String>,
    pub ingress: Vec<NpPeerRule>,
    pub egress: Vec<NpPeerRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpPeerRule {
    #[serde(default)]
    pub pod_selector: serde_json::Value,
    #[serde(default)]
    pub namespace_selector: serde_json::Value,
    #[serde(default)]
    pub ip_block: Option<NpIpBlock>,
    #[serde(default)]
    pub ports: Vec<NpPort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpIpBlock {
    pub cidr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpPort {
    pub port: u16,
    pub protocol: String,
}

/// Compile simplified NP rules into baseline_allow + deny actions.
pub fn compile_np_rules(rules: &[NpRule]) -> SyncPlan {
    let mut actions = Vec::new();
    for r in rules {
        let pod_key = format!("{}/{}", r.namespace, r.pod_selector);
        for cidr in &r.allow_cidrs {
            actions.push(PluginAction::BaselineAllow {
                pod: pod_key.clone(),
                ip: cidr.clone(),
                reason: Some(format!("np:{}", r.id)),
            });
        }
        if r.deny_all_egress {
            actions.push(PluginAction::Log {
                level: LogLevel::Info,
                message: format!("NP {} enforce egress allowlist for {}", r.id, pod_key),
            });
        }
    }
    SyncPlan {
        actions,
        routes_applied: rules.len(),
        plugins_applied: 0,
        l7_bundle: None,
    }
}

/// Compile from K8s NetworkPolicy JSON (legacy SyncPlan).
pub fn compile_k8s_np(spec: &K8sNetworkPolicySpec) -> SyncPlan {
    compile_k8s_np_rules_legacy(spec)
}

/// Full NP compile with Pod snapshot: SyncPlan + L4 policy.
pub fn compile_k8s_np_full(
    spec: &K8sNetworkPolicySpec,
    pods: &[PodSnapshot],
    namespaces: &[NamespaceSnapshot],
) -> (SyncPlan, CompiledL4Policy, IdentityMap) {
    let l4 = compile_k8s_np_l4(spec, pods, namespaces);
    let id_map = build_identity_map(pods);
    let mut plan = compile_k8s_np_rules_legacy(spec);
    for ep in &l4.endpoints {
        let pod_key = format!(
            "{}/{}",
            ep.namespace.as_deref().unwrap_or("default"),
            ep.pod_name.as_deref().unwrap_or("pod")
        );
        for rule in &l4.rules {
            if rule.action != "allow" {
                continue;
            }
            if let Some(_dst) = &rule.dst_ip {
                if rule.direction == "ingress" {
                    if let Some(src) = &rule.src_ip {
                        plan.actions.push(PluginAction::BaselineAllow {
                            pod: pod_key.clone(),
                            ip: src.clone(),
                            reason: Some(format!("np-l4:{}", spec.id)),
                        });
                    }
                }
            }
            if let Some(dst) = &rule.dst_ip {
                if rule.direction == "egress" {
                    plan.actions.push(PluginAction::BaselineAllow {
                        pod: pod_key.clone(),
                        ip: dst.clone(),
                        reason: Some(format!("np-l4:{}", spec.id)),
                    });
                }
            }
        }
    }
    (plan, l4, id_map)
}

fn compile_k8s_np_rules_legacy(spec: &K8sNetworkPolicySpec) -> SyncPlan {
    let mut rules = Vec::new();
    let selector = spec
        .pod_selector
        .get("matchLabels")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("*")))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| "app".into());

    let mut allow_cidrs = Vec::new();
    for eg in &spec.egress {
        if let Some(block) = &eg.ip_block {
            allow_cidrs.push(block.cidr.clone());
        }
    }

    rules.push(NpRule {
        id: spec.id.clone(),
        namespace: spec.namespace.clone(),
        pod_selector: selector,
        allow_cidrs,
        allow_ports: spec
            .egress
            .iter()
            .flat_map(|e| e.ports.iter().map(|p| p.port))
            .collect(),
        deny_all_egress: spec.policy_types.iter().any(|t| t == "Egress"),
    });
    compile_np_rules(&rules)
}

/// Cluster-wide: merge all NPs into a single L4 policy.
pub fn compile_all_k8s_np(
    specs: &[K8sNetworkPolicySpec],
    pods: &[PodSnapshot],
    namespaces: &[NamespaceSnapshot],
) -> (SyncPlan, CompiledL4Policy, IdentityMap) {
    let mut plans = Vec::new();
    let mut l4s = Vec::new();
    for spec in specs {
        let (plan, l4, _) = compile_k8s_np_full(spec, pods, namespaces);
        plans.push(plan);
        l4s.push(l4);
    }
    let merged_l4 = merge_l4_policies(&l4s);
    let merged_plan = merge_plans(&plans);
    let id_map = build_identity_map(pods);
    (merged_plan, merged_l4, id_map)
}

/// East-west L7 path policy → log action (Phase 4 D8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EastWestL7Policy {
    pub src_namespace: String,
    pub src_app: String,
    pub dst_namespace: String,
    pub dst_app: String,
    pub allowed_paths: Vec<String>,
}

pub fn compile_east_west_l7(policies: &[EastWestL7Policy]) -> SyncPlan {
    let mut actions = Vec::new();
    for p in policies {
        actions.push(PluginAction::Log {
            level: LogLevel::Info,
            message: format!(
                "L7 east-west policy: {}/{} -> {}/{} paths={:?}",
                p.src_namespace, p.src_app, p.dst_namespace, p.dst_app, p.allowed_paths
            ),
        });
        if p.allowed_paths.is_empty() {
            actions.push(PluginAction::Log {
                level: LogLevel::Warn,
                message: format!(
                    "east-west empty allowlist: {}/{} -> {}/{}",
                    p.src_namespace, p.src_app, p.dst_namespace, p.dst_app
                ),
            });
        }
    }
    SyncPlan {
        actions: actions.clone(),
        routes_applied: policies.len(),
        plugins_applied: 0,
        l7_bundle: None,
    }
}

/// Merge multiple SyncPlans.
pub fn merge_plans(plans: &[SyncPlan]) -> SyncPlan {
    let mut actions = Vec::new();
    let mut seen_deny = HashSet::new();
    let mut routes = 0usize;
    let mut plugins = 0usize;
    for p in plans {
        routes += p.routes_applied;
        plugins += p.plugins_applied;
        for a in &p.actions {
            if let PluginAction::DenyIp { ip, .. } = a {
                if !seen_deny.insert(ip.clone()) {
                    continue;
                }
            }
            actions.push(a.clone());
        }
    }
    SyncPlan {
        actions,
        routes_applied: routes,
        plugins_applied: plugins,
        l7_bundle: None,
    }
}

#[cfg(test)]
mod test;
