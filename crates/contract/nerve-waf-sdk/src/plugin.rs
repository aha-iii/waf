use crate::types::{PluginAction, WafRoute};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct PluginContext<'a> {
    pub route: &'a WafRoute,
    pub match_: &'a crate::types::RouteMatch,
}

pub trait WafPlugin: Send + Sync {
    fn meta(&self) -> PluginMeta;
    fn validate_config(&self, config: &serde_json::Value) -> Result<serde_json::Value, String>;
    fn apply(&self, ctx: &PluginContext<'_>, config: &serde_json::Value) -> Vec<PluginAction>;
}
