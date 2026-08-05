use serde::{Deserialize, Serialize};

/// 插件类型：rust 内置 / json 规则包 / lua 脚本
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Rust,
    Json,
    Lua,
}

impl PluginKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PluginKind::Rust => "rust",
            PluginKind::Json => "json",
            PluginKind::Lua => "lua",
        }
    }
}

/// API 返回的插件描述（含外置 JSON/Lua）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub name: String,
    pub version: String,
    pub description: String,
    pub layer: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub enabled: bool,
}

/// plugin.json 清单（plugins/l3|l7/<name>/plugin.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub layer: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default = "default_manifest_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_manifest_version() -> String {
    "1.0.0".into()
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginReloadReport {
    pub ok: bool,
    pub plugins_dir: String,
    pub json_packs: usize,
    pub lua_plugins: usize,
    pub rust_plugins: usize,
    pub total_rules: usize,
    pub errors: Vec<String>,
}
