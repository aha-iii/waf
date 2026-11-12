//! Tests extracted from `l4/l4.rs`.

use crate::l4::*;
use crate::{K8sNetworkPolicySpec, NpPeerRule, NpPort};
use std::collections::HashMap;

fn pod(ns: &str, name: &str, ip: &str, app: &str) -> PodSnapshot {
    PodSnapshot {
        namespace: ns.into(),
        name: name.into(),
        ip: ip.into(),
        labels: HashMap::from([("app".into(), app.into())]),
    }
}

#[test]
fn egress_default_deny_with_ipblock() {
    let pods = vec![
        pod("default", "frontend", "10.244.1.2", "frontend"),
        pod("default", "db", "10.244.1.3", "db"),
    ];
    let spec = K8sNetworkPolicySpec {
        id: "default/deny-all-egress".into(),
        namespace: "default".into(),
        pod_selector: serde_json::json!({"matchLabels": {"app": "frontend"}}),
        policy_types: vec!["Egress".into()],
        ingress: vec![],
        egress: vec![NpPeerRule {
            ip_block: Some(crate::NpIpBlock {
                cidr: "10.244.1.3/32".into(),
            }),
            ..Default::default()
        }],
    };
    let compiled = compile_k8s_np_l4_pods(&spec, &pods);
    assert!(compiled.endpoints[0].egress_default_deny);
    assert!(compiled.rules.iter().any(|r| {
        r.direction == "egress"
            && r.src_ip.as_deref() == Some("10.244.1.2")
            && r.dst_ip.as_deref() == Some("10.244.1.3/32")
    }));
}

#[test]
fn ingress_pod_selector_peer() {
    let pods = vec![
        pod("default", "api", "10.244.1.10", "api"),
        pod("default", "web", "10.244.1.11", "web"),
    ];
    let spec = K8sNetworkPolicySpec {
        id: "default/api-ingress".into(),
        namespace: "default".into(),
        pod_selector: serde_json::json!({"matchLabels": {"app": "api"}}),
        policy_types: vec!["Ingress".into()],
        ingress: vec![NpPeerRule {
            pod_selector: serde_json::json!({"matchLabels": {"app": "web"}}),
            ports: vec![NpPort {
                port: 8080,
                protocol: "TCP".into(),
            }],
            ..Default::default()
        }],
        egress: vec![],
    };
    let compiled = compile_k8s_np_l4_pods(&spec, &pods);
    assert!(compiled.endpoints[0].ingress_default_deny);
    assert!(compiled.rules.iter().any(|r| {
        r.direction == "ingress"
            && r.src_ip.as_deref() == Some("10.244.1.11")
            && r.dst_port == 8080
            && r.proto == "tcp"
    }));
}
