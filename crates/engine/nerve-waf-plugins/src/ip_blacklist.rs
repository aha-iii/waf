use nerve_waf_sdk::{PluginAction, PluginContext, PluginMeta, WafPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    ips: Vec<String>,
}

pub struct IpBlacklistPlugin;

impl WafPlugin for IpBlacklistPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "ip-blacklist".into(),
            version: "1.0.0".into(),
            description: "L3 IP 黑名单（TC deny map）".into(),
            priority: 10,
        }
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, String> {
        let parsed: Config =
            serde_json::from_value(config.clone()).map_err(|e| format!("invalid config: {e}"))?;
        if parsed.ips.is_empty() {
            return Err("ips must not be empty".into());
        }
        Ok(config.clone())
    }

    fn apply(&self, _ctx: &PluginContext<'_>, config: &serde_json::Value) -> Vec<PluginAction> {
        let parsed: Config =
            serde_json::from_value(config.clone()).unwrap_or(Config { ips: vec![] });
        parsed
            .ips
            .into_iter()
            .map(|ip| PluginAction::DenyIp {
                ip,
                reason: Some("ip-blacklist plugin".into()),
            })
            .collect()
    }
}
