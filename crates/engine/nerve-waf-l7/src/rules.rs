use crate::scan_targets::resolve_scan_targets;
use regex::Regex;
use serde::{Deserialize, Serialize};

fn default_paranoia_level() -> u8 {
    1
}

/// ModSecurity chain 的一步（父规则之后的子规则）。
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RawChainStep {
    pub pattern: String,
    /// 为空且 `on_matched=true` 时，在上一步命中样本上继续匹配（MATCHED_VARS）。
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
    /// ModSecurity 变量列表，如 ["ARGS", "REQUEST_FILENAME"]（chain 父规则 / 单规则）
    #[serde(default)]
    pub scan_targets: Vec<String>,
    /// 子规则步骤；全部与父规则 AND。空 = 单条规则（兼容旧 JSON）。
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
    /// 首步 pattern（展示 / 兼容）
    pub pattern: String,
    /// 首步 regex（兼容旧调用方）
    pub re: Regex,
    pub source: String,
    pub scan_targets: Vec<crate::scan_targets::ScanTarget>,
    /// 完整链：steps[0] = 父规则，其余为 chain 子步
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan_targets::ScanTarget;

    fn raw(
        id: &str,
        pattern: &str,
        category: &str,
        severity: &str,
        targets: &[&str],
        chain: Vec<RawChainStep>,
    ) -> RawRule {
        RawRule {
            id: id.into(),
            name: id.into(),
            pattern: pattern.into(),
            category: category.into(),
            severity: severity.into(),
            paranoia_level: 1,
            scan_targets: targets.iter().map(|s| (*s).into()).collect(),
            chain_steps: chain,
        }
    }

    #[test]
    fn embedded_rules_non_empty() {
        assert!(!embedded_rules().is_empty());
    }

    #[test]
    fn compiles_chain_rule() {
        let rules = compile_rules(
            vec![raw(
                "932206",
                r"\$",
                "rce",
                "high",
                &["Referer"],
                vec![
                    RawChainStep {
                        pattern: "/".into(),
                        scan_targets: vec![],
                        on_matched: true,
                    },
                    RawChainStep {
                        pattern: r"\s".into(),
                        scan_targets: vec![],
                        on_matched: true,
                    },
                ],
            )],
            "test",
        )
        .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].steps.len(), 3);
        assert!(rules[0].steps[1].on_matched);
    }

    #[test]
    fn ssrf_rule_uses_args_only_targets() {
        let rules = embedded_rules();
        let ssrf = rules.iter().find(|r| r.id == "ssrf-001").unwrap();
        assert_eq!(
            ssrf.scan_targets,
            vec![
                ScanTarget::Args,
                ScanTarget::RequestFilename,
                ScanTarget::Referer,
            ]
        );
    }
}
