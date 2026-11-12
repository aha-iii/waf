//! Host for external JSON/Lua L7 detection plugins.

mod json_loader;
mod lua_plugin;
mod registry;

pub use registry::{resolve_plugins_dir, HostedL7, PluginHost};
