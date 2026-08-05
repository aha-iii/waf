//! OWASP CRS 对齐的内置 libinjection SQLi / XSS 检测。
//!
//! 规则 ID 与 CRS `@detectSQLi` / `@detectXSS` 一致；Paranoia Level 控制默认启用范围。

mod catalog;
mod inspect;

pub use catalog::{
    effective_rule_enabled, libinjection_rule_catalog, LibInjectionRuleDef, ScanTarget,
};
pub use inspect::inspect_libinjection;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 单条 libinjection 规则开关（覆盖 Paranoia 默认行为）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LibInjectionRuleSwitch {
    /// `None` = 跟随 Paranoia；`Some(false)` 强制关；`Some(true)` 强制开（仍受总开关与模块开关约束）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibInjectionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// CRS Paranoia Level 上限（1–4）。规则 PL ≤ 此值时默认启用。
    #[serde(default = "default_paranoia")]
    pub paranoia_max: u8,
    /// 按规则 ID 覆盖（942100 / 942101 / 941100 / 941101）
    #[serde(default)]
    pub rules: HashMap<String, LibInjectionRuleSwitch>,
}

fn default_true() -> bool {
    true
}

fn default_paranoia() -> u8 {
    1
}

impl Default for LibInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            paranoia_max: 1,
            rules: HashMap::new(),
        }
    }
}

/// API / Console 展示用：规则目录 + 当前生效状态
#[derive(Debug, Clone, Serialize)]
pub struct LibInjectionRuleStatus {
    pub id: String,
    pub name: String,
    pub category: String,
    pub paranoia_level: u8,
    pub severity: String,
    pub scan_targets: Vec<String>,
    pub crs_operator: String,
    /// 在 paranoia_max=N 时 CRS 标准是否启用该规则
    pub standard_at_pl: [bool; 4],
    pub override_enabled: Option<bool>,
    pub effective_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibInjectionStatus {
    pub enabled: bool,
    pub paranoia_max: u8,
    pub library_version: String,
    pub rules: Vec<LibInjectionRuleStatus>,
    pub active_count: usize,
}

impl LibInjectionConfig {
    pub fn status(&self) -> LibInjectionStatus {
        let catalog = libinjection_rule_catalog();
        let rules: Vec<LibInjectionRuleStatus> = catalog
            .iter()
            .map(|def| {
                let standard_at_pl: [bool; 4] = [
                    def.paranoia_level <= 1,
                    def.paranoia_level <= 2,
                    def.paranoia_level <= 3,
                    def.paranoia_level <= 4,
                ];
                let override_enabled = self.rules.get(def.id).and_then(|s| s.enabled);
                let effective_enabled =
                    self.enabled && effective_rule_enabled(self, def, self.paranoia_max);
                LibInjectionRuleStatus {
                    id: def.id.into(),
                    name: def.name.into(),
                    category: def.category.into(),
                    paranoia_level: def.paranoia_level,
                    severity: def.severity.into(),
                    scan_targets: def
                        .scan_targets
                        .iter()
                        .map(|t| t.label().into())
                        .collect(),
                    crs_operator: def.crs_operator.into(),
                    standard_at_pl,
                    override_enabled,
                    effective_enabled,
                }
            })
            .collect();
        let active_count = rules.iter().filter(|r| r.effective_enabled).count();
        LibInjectionStatus {
            enabled: self.enabled,
            paranoia_max: self.paranoia_max,
            library_version: "libinjectionrs-0.1.1".into(),
            rules,
            active_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pl1_enables_core_rules_only() {
        let cfg = LibInjectionConfig {
            enabled: true,
            paranoia_max: 1,
            ..Default::default()
        };
        let st = cfg.status();
        let r942100 = st.rules.iter().find(|r| r.id == "942100").unwrap();
        let r942101 = st.rules.iter().find(|r| r.id == "942101").unwrap();
        assert!(r942100.effective_enabled);
        assert!(!r942101.effective_enabled);
        assert!(r942100.standard_at_pl[0]);
        assert!(!r942101.standard_at_pl[0]);
        assert!(r942101.standard_at_pl[1]);
    }
}
