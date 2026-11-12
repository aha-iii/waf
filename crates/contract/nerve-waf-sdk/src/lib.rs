//! Shared WAF SDK types: routes, events, plugins, protection modules, and config shapes.

mod config;
mod events;
mod l7_checks;
mod modules;
mod plugin;
mod plugin_manifest;
mod sensitivity;
mod trace;
mod traffic;
mod types;

pub use l7_checks::*;

pub use config::{
    ApiConfig, ConfigError, ConsoleConfig, DatabaseConfig, WafFileConfig, WafLoggingConfig,
    WafNodeConfig,
};
pub use events::*;
pub use modules::*;
pub use plugin::{PluginContext, PluginMeta, WafPlugin};
pub use plugin_manifest::*;
pub use sensitivity::{rule_matches_sensitivity, sensitivity_paranoia_max, severity_rank};
pub use trace::*;
pub use traffic::*;
pub use types::*;

#[cfg(test)]
mod test;
