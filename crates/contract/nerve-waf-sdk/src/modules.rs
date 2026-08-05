use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_modules_version() -> u32 {
    1
}

fn default_mode() -> String {
    "block".into()
}

fn default_action() -> String {
    "block".into()
}

fn default_sensitivity() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: String,
    /// 应用级关闭的单条规则 ID（全局规则库仍保留，仅本应用不命中）
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ModuleConfig {
    pub fn is_rule_disabled(&self, rule_id: &str) -> bool {
        self.disabled_rules.iter().any(|id| id == rule_id)
    }

    /// 比较模块生效配置（含本应用关闭的规则列表与扩展参数）。
    pub fn equals_effective(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.action == other.action
            && self.sensitivity == other.sensitivity
            && self.disabled_rules == other.disabled_rules
            && self.extra == other.extra
    }

    pub fn extra_u32(&self, key: &str) -> Option<u32> {
        self.extra
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
    }

    pub fn extra_u64(&self, key: &str) -> Option<u64> {
        self.extra.get(key).and_then(|v| v.as_u64())
    }
}

fn default_enabled() -> bool {
    true
}

impl Default for ModuleConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            action: default_action(),
            sensitivity: default_sensitivity(),
            disabled_rules: Vec::new(),
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtectionModules {
    #[serde(default = "default_modules_version")]
    pub version: u32,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub modules: HashMap<String, ModuleConfig>,
}

impl ProtectionModules {
    pub fn is_rule_disabled(&self, category: &str, rule_id: &str) -> bool {
        self.modules
            .get(category)
            .map(|m| m.is_rule_disabled(rule_id))
            .unwrap_or(false)
    }

    /// 合并全局 CC 引擎配置与模块级覆盖（策略 / 应用 modules.cc）。
    pub fn merge_cc_config(&self, global: &nerve_waf_cc::CcConfig) -> nerve_waf_cc::CcConfig {
        let mut out = global.clone();
        let Some(m) = self.modules.get("cc") else {
            return out;
        };
        if !m.enabled {
            out.enabled = false;
        }
        if let Some(v) = m.extra_u32("requests_per_minute") {
            out.requests_per_minute = v;
        }
        if let Some(v) = m.extra_u32("burst") {
            out.burst = v;
        }
        if let Some(v) = m.extra_u64("block_duration_sec") {
            out.block_duration_sec = v;
        }
        out
    }

    /// 合并全局 Bot 配置与模块级覆盖。
    pub fn merge_bot_config(&self, global: &nerve_waf_bot::BotConfig) -> nerve_waf_bot::BotConfig {
        let mut out = global.clone();
        let Some(m) = self.modules.get("bot") else {
            return out;
        };
        out.enabled = global.enabled && m.enabled;
        if !m.action.is_empty() {
            out.action = m.action.clone();
        }
        if let Some(v) = m.extra.get("score_threshold").and_then(|v| v.as_u64()) {
            out.score_threshold = v as u8;
        }
        out
    }

    /// 将 CC/Bot 生效参数写入 modules 快照（新建策略 / 套用前固化，与全局运行时隔离）。
    pub fn materialize_basic_module_params(
        &mut self,
        cc: &nerve_waf_cc::CcConfig,
        bot: &nerve_waf_bot::BotConfig,
    ) {
        let cc_mod = self.modules.entry("cc".into()).or_default();
        cc_mod
            .extra
            .insert("requests_per_minute".into(), serde_json::json!(cc.requests_per_minute));
        cc_mod
            .extra
            .insert("burst".into(), serde_json::json!(cc.burst));
        cc_mod.extra.insert(
            "block_duration_sec".into(),
            serde_json::json!(cc.block_duration_sec),
        );

        let bot_mod = self.modules.entry("bot".into()).or_default();
        bot_mod
            .extra
            .insert("score_threshold".into(), serde_json::json!(bot.score_threshold));
    }

    /// 将应用级覆盖合并到全局默认模板上，得到生效配置。
    pub fn merge_with_overrides(base: &Self, overrides: &Self) -> Self {
        let mut out = base.clone();
        if overrides.mode != base.mode {
            out.mode = overrides.mode.clone();
        }
        if overrides.version != base.version {
            out.version = overrides.version;
        }
        for (key, cfg) in &overrides.modules {
            out.modules.insert(key.clone(), cfg.clone());
        }
        out
    }

    /// 相对全局默认计算应用级差异；无差异时返回 `None`（不应落库）。
    pub fn diff_as_overrides(base: &Self, effective: &Self) -> Option<Self> {
        let mut modules = HashMap::new();
        let mut keys: std::collections::HashSet<_> = base.modules.keys().collect();
        keys.extend(effective.modules.keys());
        for key in keys {
            let b = base.modules.get(key).cloned().unwrap_or_default();
            let e = effective.modules.get(key).cloned().unwrap_or_default();
            if !b.equals_effective(&e) {
                modules.insert(key.clone(), e);
            }
        }
        let mode_diff = effective.mode != base.mode;
        if modules.is_empty() && !mode_diff {
            return None;
        }
        Some(Self {
            version: effective.version,
            mode: effective.mode.clone(),
            modules,
        })
    }
}

impl Default for ProtectionModules {
    fn default() -> Self {
        let mut modules = HashMap::new();
        for key in [
            "sqli",
            "xss",
            "rce",
            "path_traversal",
            "ssrf",
            "file_upload",
            "webshell",
            "info_leak",
            "bot",
            "cc",
            "custom",
        ] {
            modules.insert(
                key.into(),
                ModuleConfig {
                    enabled: matches!(
                        key,
                        "sqli" | "xss" | "rce" | "path_traversal" | "cc" | "custom"
                    ),
                    action: "block".into(),
                    sensitivity: "medium".into(),
                    disabled_rules: Vec::new(),
                    extra: HashMap::new(),
                },
            );
        }
        Self {
            version: 1,
            mode: "block".into(),
            modules,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_and_diff_roundtrip() {
        let mut base = ProtectionModules::default();
        base.modules.get_mut("sqli").unwrap().enabled = true;
        base.modules.get_mut("webshell").unwrap().enabled = false;

        let mut effective = base.clone();
        effective.modules.get_mut("webshell").unwrap().enabled = true;
        effective
            .modules
            .get_mut("sqli")
            .unwrap()
            .disabled_rules
            .push("942100".into());

        let ov = ProtectionModules::diff_as_overrides(&base, &effective).expect("has overrides");
        assert!(ov.modules.contains_key("webshell"));
        assert!(ov.modules.contains_key("sqli"));
        let merged = ProtectionModules::merge_with_overrides(&base, &ov);
        assert_eq!(merged, effective);
    }

    #[test]
    fn diff_none_when_identical() {
        let base = ProtectionModules::default();
        assert!(ProtectionModules::diff_as_overrides(&base, &base).is_none());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct L7Bundle {
    pub modules: ProtectionModules,
    pub rule_count: usize,
}
