use nerve_waf_sdk::{PluginMeta, WafPlugin};
use std::sync::Arc;

use crate::baseline_allow::BaselineAllowPlugin;
use crate::ip_blacklist::IpBlacklistPlugin;
use crate::ip_whitelist::IpWhitelistPlugin;
use crate::port_block::PortBlockPlugin;

pub fn plugin_registry() -> Vec<Arc<dyn WafPlugin>> {
    vec![
        Arc::new(IpWhitelistPlugin),
        Arc::new(IpBlacklistPlugin),
        Arc::new(PortBlockPlugin),
        Arc::new(BaselineAllowPlugin),
    ]
}

pub fn list_plugins() -> Vec<PluginMeta> {
    let mut plugins: Vec<_> = plugin_registry()
        .iter()
        .map(|p| p.meta())
        .collect();
    plugins.sort_by_key(|p| p.priority);
    plugins
}

pub fn get_plugin(name: &str) -> Option<Arc<dyn WafPlugin>> {
    plugin_registry()
        .into_iter()
        .find(|p| p.meta().name == name)
}
