//! libinjection-based SQLi/XSS detectors aligned with OWASP CRS rule IDs.

mod catalog;
mod inspect;

pub use catalog::{
    effective_rule_enabled, libinjection_rule_catalog, LibInjectionRuleDef, ScanTarget,
};
pub use inspect::inspect_libinjection;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-rule libinjection switch (overrides Paranoia defaults).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LibInjectionRuleSwitch {
    /// `None` = follow Paranoia; `Some(false)` force off; `Some(true)` force on (still gated by master/module switches).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibInjectionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// CRS Paranoia Level cap (1–4). Rules with PL ≤ this value enabled by default.
    #[serde(default = "default_paranoia")]
    pub paranoia_max: u8,
    /// Per-rule-id overrides (942100 / 942101 / 941100 / 941101).
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

/// For API / Console: rule catalog + current effective state.
#[derive(Debug, Clone, Serialize)]
pub struct LibInjectionRuleStatus {
    pub id: String,
    pub name: String,
    pub category: String,
    pub paranoia_level: u8,
    pub severity: String,
    pub scan_targets: Vec<String>,
    pub crs_operator: String,
    /// Whether CRS standard enables the rule when paranoia_max=N.
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
mod test;
