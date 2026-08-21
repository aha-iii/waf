use crate::scan_targets::resolve_scan_targets;
use regex::Regex;
use serde::{Deserialize, Serialize};

fn default_paranoia_level() -> u8 {
    1
}

/// One step of a ModSecurity chain (child after parent).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RawChainStep {
    pub pattern: String,
    /// Empty with `on_matched=true` continues matching on prior hit sample (MATCHED_VARS).
    #[serde(default)]
    pub scan_targets: Vec<String>,
    #[serde(default)]
    pub on_matched: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub category: String,
    pub severity: String,
    #[serde(default = "default_paranoia_level")]
    pub paranoia_level: u8,
    /// ModSecurity variables, e.g. ["ARGS", "REQUEST_FILENAME"] (chain parent / single rule).
    #[serde(default)]
    pub scan_targets: Vec<String>,
    /// Child steps; all AND with parent. Empty = single rule (legacy JSON compatible).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain_steps: Vec<RawChainStep>,
}

#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub id: String,
    pub name: String,
    pub category: String,
    pub severity: String,
    pub paranoia_level: u8,
    /// First-step pattern (display / compat).
    pub pattern: String,
    /// First-step regex (legacy callers).
    pub re: Regex,
    pub source: String,
    pub scan_targets: Vec<crate::scan_targets::ScanTarget>,
    /// Full chain: steps[0] = parent; rest are chain children.
    pub steps: Vec<CompiledStep>,
}

#[derive(Debug, Clone)]
pub struct CompiledStep {
    pub pattern: String,
    pub re: Regex,
    pub scan_targets: Vec<crate::scan_targets::ScanTarget>,
    pub on_matched: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleCompileError {
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid regex in rule {id}: {message}")]
    Regex { id: String, message: String },
}

pub fn compile_rules_from_json(raw: &str, source: &str) -> Result<Vec<CompiledRule>, RuleCompileError> {
    let rules: Vec<RawRule> = serde_json::from_str(raw)?;
    compile_rules(rules, source)
}

pub fn compile_rules(rules: Vec<RawRule>, source: &str) -> Result<Vec<CompiledRule>, RuleCompileError> {
    let mut out = Vec::with_capacity(rules.len());
    for r in rules {
        let mut steps = Vec::with_capacity(1 + r.chain_steps.len());
        let first_re = Regex::new(&r.pattern).map_err(|e| RuleCompileError::Regex {
            id: r.id.clone(),
            message: e.to_string(),
        })?;
        let first_targets = resolve_scan_targets(Some(&r.scan_targets));
        steps.push(CompiledStep {
            pattern: r.pattern.clone(),
            re: first_re.clone(),
            scan_targets: first_targets.clone(),
            on_matched: false,
        });
        for (idx, step) in r.chain_steps.iter().enumerate() {
            let re = Regex::new(&step.pattern).map_err(|e| RuleCompileError::Regex {
                id: format!("{}#chain{}", r.id, idx + 1),
                message: e.to_string(),
            })?;
            let targets = if step.on_matched {
                Vec::new()
            } else {
                resolve_scan_targets(Some(&step.scan_targets))
            };
            steps.push(CompiledStep {
                pattern: step.pattern.clone(),
                re,
                scan_targets: targets,
                on_matched: step.on_matched,
            });
        }
        out.push(CompiledRule {
            id: r.id,
            name: r.name,
            category: r.category,
            severity: r.severity,
            paranoia_level: r.paranoia_level,
            pattern: steps[0].pattern.clone(),
            re: first_re,
            source: source.to_string(),
            scan_targets: first_targets,
            steps,
        });
    }
    Ok(out)
}

pub fn embedded_rules() -> Vec<CompiledRule> {
    let mut out = Vec::new();
    for (file, raw) in [
        ("embedded/sqli.json", include_str!("../rules/sqli.json")),
        ("embedded/xss.json", include_str!("../rules/xss.json")),
        ("embedded/misc.json", include_str!("../rules/misc.json")),
    ] {
        if let Ok(rules) = compile_rules_from_json(raw, file) {
            out.extend(rules);
        }
    }
    out
}
