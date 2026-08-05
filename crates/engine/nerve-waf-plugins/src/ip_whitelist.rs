use nerve_waf_sdk::{PluginAction, PluginContext, PluginMeta, WafPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    ips: Vec<String>,
}

pub struct IpWhitelistPlugin;

impl WafPlugin for IpWhitelistPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "ip-whitelist".into(),
            version: "1.0.0".into(),
            description: "L3 解封 IP（从黑名单移除）".into(),
            priority: 5,
        }
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, String> {
        let parsed: Config = serde_json::from_value(config.clone())
            .map_err(|e| format!("invalid config: {e}"))?;
        if parsed.ips.is_empty() {
            return Err("ips must not be empty".into());
        }
        Ok(config.clone())
    }

    fn apply(&self, _ctx: &PluginContext<'_>, config: &serde_json::Value) -> Vec<PluginAction> {
        let parsed: Config = serde_json::from_value(config.clone()).unwrap_or(Config { ips: vec![] });
        parsed
            .ips
            .into_iter()
            .map(|ip| PluginAction::AllowIp {
                ip,
                reason: Some("ip-whitelist plugin".into()),
            })
            .collect()
    }
}
