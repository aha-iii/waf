use nerve_waf_sdk::{PluginAction, PluginContext, PluginMeta, WafPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    ports: Vec<u16>,
    #[serde(default = "default_deny_dst")]
    deny_dst: bool,
    #[serde(default)]
    block_ips: Vec<String>,
}

fn default_deny_dst() -> bool {
    true
}

pub struct PortBlockPlugin;

impl WafPlugin for PortBlockPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "port-block".into(),
            version: "1.0.0".into(),
            description: "可疑端口 Escalation 规则（编译为 deny）".into(),
            priority: 20,
        }
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, String> {
        let parsed: Config = serde_json::from_value(config.clone())
            .map_err(|e| format!("invalid config: {e}"))?;
        if parsed.ports.is_empty() && parsed.block_ips.is_empty() {
            return Err("ports or block_ips required".into());
        }
        Ok(config.clone())
    }

    fn apply(&self, ctx: &PluginContext<'_>, config: &serde_json::Value) -> Vec<PluginAction> {
        let parsed: Config = match serde_json::from_value(config.clone()) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut actions = Vec::new();
        let match_port = ctx.match_.dst_port;

        if let Some(port) = match_port {
            if parsed.ports.contains(&port) && parsed.deny_dst {
                if let Some(dst_ip) = &ctx.match_.dst_ip {
                    actions.push(PluginAction::DenyIp {
                        ip: dst_ip.clone(),
                        reason: Some(format!("port-block:{port}")),
                    });
                }
            }
        }

        for ip in &parsed.block_ips {
            actions.push(PluginAction::DenyIp {
                ip: ip.clone(),
                reason: Some(if match_port.is_some() {
                    "port-block:fixed".into()
                } else {
                    "port-block".into()
                }),
            });
        }

        actions
    }
}
