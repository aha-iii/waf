//! Tests extracted from `lib.rs`.

use crate::*;
use nerve_waf_sdk::{PluginAction, RoutePluginRef, WafRoute};

#[test]
fn compiles_example_blacklist() {
    let config = WafConfig {
        version: 1,
        routes: vec![WafRoute {
            id: "manual-blacklist".into(),
            name: "手动黑名单".into(),
            priority: 200,
            enabled: true,
            match_: Default::default(),
            plugins: vec![RoutePluginRef {
                name: "ip-blacklist".into(),
                enabled: true,
                config: serde_json::json!({ "ips": ["1.2.3.4"] }),
            }],
            layer: "l3".into(),
            upstream_id: None,
        }],
    };
        let plan = compile_config(&config, &ProtectionModules::default(), nerve_waf_l7::embedded_rules().len());
        assert_eq!(plan.routes_applied, 1);
        assert_eq!(plan.plugins_applied, 1);
        assert!(plan.actions.iter().any(|a| matches!(
            a,
            PluginAction::DenyIp { ip, .. } if ip == "1.2.3.4"
        )));
    }

    #[test]
    fn merges_runtime_denies() {
        let config = WafConfig::default();
        let plan = compile_config_with_runtime(
            &config,
            &ProtectionModules::default(),
            0,
            &["1.2.3.5".into()],
        );
        assert!(plan.actions.iter().any(|a| matches!(
            a,
            PluginAction::DenyIp { ip, reason, .. }
                if ip == "1.2.3.5" && reason.as_deref() == Some("runtime deny overlay")
        )));
    }

    #[test]
    fn compiles_baseline_allow() {
        let config = WafConfig {
            version: 1,
            routes: vec![WafRoute {
                id: "pod-dns-whitelist".into(),
                name: "Pod DNS 基线加白".into(),
                priority: 50,
                enabled: true,
                match_: nerve_waf_sdk::RouteMatch {
                    pod: Some("cgroup:*".into()),
                    ..Default::default()
                },
                plugins: vec![RoutePluginRef {
                    name: "baseline-allow".into(),
                    enabled: true,
                    config: serde_json::json!({
                        "pod": "cgroup:1da40000",
                        "ips": ["8.8.8.8", "1.1.1.1"]
                    }),
                }],
                layer: "l3".into(),
                upstream_id: None,
            }],
        };
        let plan = compile_config(&config, &ProtectionModules::default(), 0);
        let baseline: Vec<_> = plan
            .actions
            .iter()
            .filter_map(|a| match a {
                PluginAction::BaselineAllow { pod, ip, .. } => Some((pod.as_str(), ip.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(baseline.len(), 2);
        assert!(baseline.contains(&("cgroup:1da40000", "8.8.8.8")));
        assert!(baseline.contains(&("cgroup:1da40000", "1.1.1.1")));
    }

    #[test]
    fn compiles_port_block_with_block_ips() {
        let config = WafConfig {
            version: 1,
            routes: vec![WafRoute {
                id: "block-malicious-egress".into(),
                name: "封禁恶意出站 IP".into(),
                priority: 100,
                enabled: true,
                match_: nerve_waf_sdk::RouteMatch {
                    dst_port: Some(4444),
                    ..Default::default()
                },
                plugins: vec![RoutePluginRef {
                    name: "port-block".into(),
                    enabled: true,
                    config: serde_json::json!({
                        "ports": [4444, 6666, 31337],
                        "deny_dst": true,
                        "block_ips": ["203.0.113.66"]
                    }),
                }],
                layer: "l3".into(),
                upstream_id: None,
            }],
        };
        let plan = compile_config(&config, &ProtectionModules::default(), 0);
        assert!(plan.actions.iter().any(|a| matches!(
            a,
            PluginAction::DenyIp { ip, reason, .. }
                if ip == "203.0.113.66" && reason.as_deref() == Some("port-block")
        )));
    }

    #[test]
    fn compiles_port_block_with_dst_ip_in_match() {
        let config = WafConfig {
            version: 1,
            routes: vec![WafRoute {
                id: "block-port-target".into(),
                name: "按端口+目标 IP 封禁".into(),
                priority: 100,
                enabled: true,
                match_: nerve_waf_sdk::RouteMatch {
                    dst_port: Some(4444),
                    dst_ip: Some("198.51.100.10".into()),
                    ..Default::default()
                },
                plugins: vec![RoutePluginRef {
                    name: "port-block".into(),
                    enabled: true,
                    config: serde_json::json!({ "ports": [4444], "deny_dst": true }),
                }],
                layer: "l3".into(),
                upstream_id: None,
            }],
        };
        let plan = compile_config(&config, &ProtectionModules::default(), 0);
        assert!(plan.actions.iter().any(|a| matches!(
            a,
            PluginAction::DenyIp { ip, reason, .. }
                if ip == "198.51.100.10" && reason.as_deref() == Some("port-block:4444")
        )));
    }

    #[test]
    fn compiles_console_l3_routes_and_sync_body() {
        let raw = include_str!("../../../../../examples/routes.json");
        let config: WafConfig = serde_json::from_str(raw).expect("examples/routes.json");
        let plan = compile_config_with_runtime(
            &config,
            &ProtectionModules::default(),
            0,
            &["1.2.3.5".into()],
        );
        assert_eq!(plan.routes_applied, 3);
        assert_eq!(plan.plugins_applied, 3);

        assert!(plan.actions.iter().any(|a| matches!(
            a, PluginAction::DenyIp { ip, .. } if ip == "1.2.3.4"
        )));
        assert!(plan.actions.iter().any(|a| matches!(
            a, PluginAction::BaselineAllow { ip, .. } if ip == "8.8.8.8"
        )));
        assert!(plan.actions.iter().any(|a| matches!(
            a,
            PluginAction::DenyIp { ip, reason, .. }
                if ip == "1.2.3.5" && reason.as_deref() == Some("runtime deny overlay")
        )));

        let body = sync_plan_json(&plan);
        let deny = body["deny"].as_array().expect("deny array");
        let baseline = body["baseline"].as_array().expect("baseline array");
        assert!(deny.iter().any(|v| v.as_str() == Some("1.2.3.4")));
        assert!(deny.iter().any(|v| v.as_str() == Some("1.2.3.5")));
        assert!(baseline.iter().any(|b| {
            b.get("pod").and_then(|v| v.as_str()) == Some("cgroup:1da40000")
                && b.get("ips")
                    .and_then(|v| v.as_array())
                    .map(|ips| ips.iter().any(|ip| ip.as_str() == Some("8.8.8.8")))
                    .unwrap_or(false)
        }));
    }
