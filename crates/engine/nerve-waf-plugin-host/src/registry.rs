use crate::json_loader::{discover_manifest_dirs, load_rules_from_manifest};
use crate::lua_plugin::LuaInspectPlugin;
use nerve_waf_l7::{embedded_rules, CompiledRule, InspectRequest, InspectResult, L7Engine};
use nerve_waf_plugins::list_plugins;
use nerve_waf_sdk::{
    PluginDescriptor, PluginManifest, PluginMeta, PluginReloadReport, ProtectionModules,
};
use std::path::{Path, PathBuf};

/// 解析 plugins 目录：环境变量 > data/../plugins > cwd/plugins > 仓库默认
pub fn resolve_plugins_dir(data_dir: Option<&Path>) -> PathBuf {
    if let Ok(p) = std::env::var("NERVE_WAF_PLUGINS") {
        return PathBuf::from(p);
    }
    if let Some(data) = data_dir {
        let candidate = data.join("../plugins");
        if candidate.is_dir() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }
    let candidates = [
        PathBuf::from("plugins"),
        PathBuf::from("../plugins"),
        PathBuf::from("../../plugins"),
    ];
    for c in candidates {
        if c.is_dir() {
            return c.canonicalize().unwrap_or(c);
        }
    }
    PathBuf::from("plugins")
}

struct JsonRulePack {
    manifest: PluginManifest,
    dir: PathBuf,
    rules: Vec<CompiledRule>,
}

pub struct PluginHost {
    plugins_dir: PathBuf,
    json_packs: Vec<JsonRulePack>,
    lua_plugins: Vec<LuaInspectPlugin>,
    errors: Vec<String>,
}

impl PluginHost {
    pub fn new(plugins_dir: PathBuf) -> Self {
        let mut host = Self {
            plugins_dir,
            json_packs: Vec::new(),
            lua_plugins: Vec::new(),
            errors: Vec::new(),
        };
        host.reload();
        host
    }

    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    pub fn reload(&mut self) -> PluginReloadReport {
        self.json_packs.clear();
        self.lua_plugins.clear();
        self.errors.clear();

        if !self.plugins_dir.is_dir() {
            self.errors.push(format!(
                "plugins 目录不存在: {}（将仅使用内置 Rust/embedded 规则）",
                self.plugins_dir.display()
            ));
        } else {
            self.scan_layer("l7");
        }

        let rust_count = list_plugins().len();
        PluginReloadReport {
            ok: self.errors.is_empty(),
            plugins_dir: self.plugins_dir.display().to_string(),
            json_packs: self.json_packs.len(),
            lua_plugins: self.lua_plugins.len(),
            rust_plugins: rust_count,
            total_rules: self.total_rule_count(),
            errors: self.errors.clone(),
        }
    }

    fn scan_layer(&mut self, layer: &str) {
        for dir in discover_manifest_dirs(&self.plugins_dir, layer) {
            let manifest_path = dir.join("plugin.json");
            let raw = match std::fs::read_to_string(&manifest_path) {
                Ok(r) => r,
                Err(e) => {
                    self.errors
                        .push(format!("read {}: {e}", manifest_path.display()));
                    continue;
                }
            };
            let manifest: PluginManifest = match serde_json::from_str(&raw) {
                Ok(m) => m,
                Err(e) => {
                    self.errors
                        .push(format!("parse {}: {e}", manifest_path.display()));
                    continue;
                }
            };
            if !manifest.enabled {
                continue;
            }
            let ptype = manifest.plugin_type.to_lowercase();
            match ptype.as_str() {
                "json" => match load_rules_from_manifest(&dir, &manifest.rules) {
                    Ok(rules) => {
                        if rules.is_empty() {
                            self.errors.push(format!(
                                "json plugin {} loaded 0 rules from {}",
                                manifest.name,
                                dir.display()
                            ));
                        }
                        self.json_packs.push(JsonRulePack {
                            manifest,
                            dir,
                            rules,
                        });
                    }
                    Err(e) => self.errors.push(e),
                },
                "lua" => match LuaInspectPlugin::from_manifest(&dir, manifest) {
                    Ok(p) => self.lua_plugins.push(p),
                    Err(e) => self.errors.push(e),
                },
                other => self.errors.push(format!(
                    "unsupported plugin type '{}' in {}",
                    other,
                    manifest_path.display()
                )),
            }
        }
    }

    pub fn total_rule_count(&self) -> usize {
        let json_rules: usize = self.json_packs.iter().map(|p| p.rules.len()).sum();
        if json_rules > 0 {
            json_rules
        } else {
            embedded_rules().len()
        }
    }

    pub fn build_l7_engine(&self, modules: &ProtectionModules) -> L7Engine {
        self.build_merged_engine(modules, Vec::new())
    }

    /// 合并规则 precedence：embedded 基线 → JSON 插件包 → CRS/AI（与 API `merged_l7_rules_internal` 一致）
    pub fn build_merged_engine(
        &self,
        modules: &ProtectionModules,
        crs_rules: Vec<CompiledRule>,
    ) -> L7Engine {
        let mut rules = embedded_rules();
        for pack in &self.json_packs {
            rules.extend(pack.rules.clone());
        }
        rules.extend(crs_rules);
        rules = dedupe_rules(rules);
        L7Engine::from_rules(modules.clone(), rules)
    }

    pub fn list_descriptors(&self) -> Vec<PluginDescriptor> {
        let mut out: Vec<PluginDescriptor> = list_plugins()
            .into_iter()
            .map(rust_meta_to_descriptor)
            .collect();

        for pack in &self.json_packs {
            out.push(PluginDescriptor {
                name: pack.manifest.name.clone(),
                version: pack.manifest.version.clone(),
                description: pack.manifest.description.clone(),
                layer: pack.manifest.layer.clone(),
                plugin_type: "json".into(),
                priority: pack.manifest.priority,
                category: pack.manifest.category.clone(),
                rule_count: Some(pack.rules.len()),
                path: Some(pack.dir.display().to_string()),
                enabled: pack.manifest.enabled,
            });
        }
        for lp in &self.lua_plugins {
            out.push(PluginDescriptor {
                name: lp.manifest.name.clone(),
                version: lp.manifest.version.clone(),
                description: lp.manifest.description.clone(),
                layer: lp.manifest.layer.clone(),
                plugin_type: "lua".into(),
                priority: lp.manifest.priority,
                category: lp.manifest.category.clone(),
                rule_count: None,
                path: Some(lp.dir.display().to_string()),
                enabled: lp.manifest.enabled,
            });
        }
        out.sort_by_key(|d| (d.layer.clone(), d.priority, d.name.clone()));
        out
    }

    pub fn last_errors(&self) -> &[String] {
        &self.errors
    }
}

fn rust_meta_to_descriptor(meta: PluginMeta) -> PluginDescriptor {
    PluginDescriptor {
        name: meta.name,
        version: meta.version,
        description: meta.description,
        layer: "l3".into(),
        plugin_type: "rust".into(),
        priority: meta.priority,
        category: None,
        rule_count: None,
        path: Some("crates/nerve-waf-plugins".into()),
        enabled: true,
    }
}

/// L7 检测：JSON 规则引擎 + Lua 插件链
pub struct HostedL7 {
    engine: L7Engine,
    lua_plugins: Vec<LuaInspectPlugin>,
}

impl HostedL7 {
    pub fn from_host(
        host: &PluginHost,
        modules: &ProtectionModules,
        crs_rules: Vec<CompiledRule>,
    ) -> Self {
        Self {
            engine: host.build_merged_engine(modules, crs_rules),
            lua_plugins: host.lua_plugins.clone(),
        }
    }

    /// 兼容旧调用：仅插件规则，不含 CRS
    pub fn from_host_plugins_only(host: &PluginHost, modules: &ProtectionModules) -> Self {
        Self::from_host(host, modules, Vec::new())
    }

    pub fn engine(&self) -> &L7Engine {
        &self.engine
    }

    pub fn set_external_rules(&mut self, rules: Vec<CompiledRule>) {
        let mut merged = self.engine.rules().to_vec();
        merged.extend(rules);
        merged = dedupe_rules(merged);
        self.engine.set_rules(merged);
    }

    pub fn inspect(&self, req: &InspectRequest) -> Option<InspectResult> {
        if let Some(hit) = self.engine.inspect_upload(req) {
            return Some(hit);
        }
        if let Some(hit) = self.engine.inspect(req) {
            return Some(hit);
        }
        let mode = self.engine.modules().mode.clone();
        for lp in &self.lua_plugins {
            match lp.inspect(req, &mode) {
                Ok(Some(hit)) => return Some(hit),
                Ok(None) => {}
                Err(e) => tracing::warn!("lua plugin {} error: {e}", lp.manifest.name),
            }
        }
        None
    }

    pub fn rule_count(&self) -> usize {
        self.engine.rule_count()
    }

    pub fn lua_plugins(&self) -> &[LuaInspectPlugin] {
        &self.lua_plugins
    }
}

// LuaInspectPlugin needs Clone for HostedL7 - add derive manually via clone in from_host
fn dedupe_rules(rules: Vec<CompiledRule>) -> Vec<CompiledRule> {
    let mut seen = std::collections::HashSet::new();
    rules
        .into_iter()
        .filter(|r| seen.insert(r.id.clone()))
        .collect()
}

impl Clone for LuaInspectPlugin {
    fn clone(&self) -> Self {
        Self {
            manifest: self.manifest.clone(),
            dir: self.dir.clone(),
            script_path: self.script_path.clone(),
        }
    }
}
