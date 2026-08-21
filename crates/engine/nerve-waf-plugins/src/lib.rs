//! Built-in WAF-L3 Rust plugins (IP blacklist, port block, and related).

mod baseline_allow;
mod ip_blacklist;
mod ip_whitelist;
mod port_block;
mod registry;

pub use registry::{get_plugin, list_plugins, plugin_registry};
