use nerve_waf_sdk::{PluginAction, PluginContext, PluginMeta, WafPlugin};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    pod: String,
    ips: Vec<String>,
}

pub struct BaselineAllowPlugin;

impl WafPlugin for BaselineAllowPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "baseline-allow".into(),
            version: "1.0.0".into(),
            description: "行为基线出站白名单".into(),
            priority: 30,
        }
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, String> {
        let parsed: Config =
            serde_json::from_value(config.clone()).map_err(|e| format!("invalid config: {e}"))?;
        if parsed.pod.is_empty() {
            return Err("pod required".into());
        }
        if parsed.ips.is_empty() {
            return Err("ips must not be empty".into());
        }
        Ok(config.clone())
    }

    fn apply(&self, _ctx: &PluginContext<'_>, config: &serde_json::Value) -> Vec<PluginAction> {
        let parsed: Config = match serde_json::from_value(config.clone()) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        parsed
            .ips
            .into_iter()
            .map(|ip| PluginAction::BaselineAllow {
                pod: parsed.pod.clone(),
                ip,
                reason: Some("baseline-allow plugin".into()),
            })
            .collect()
    }
}
