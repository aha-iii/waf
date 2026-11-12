use crate::{InspectRequest, InspectResult};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomRule {
    pub id: String,
    pub rule_set_id: String,
    pub name: String,
    #[serde(default = "default_phase")]
    pub phase: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub match_field: String,
    pub match_op: String,
    pub match_pattern: String,
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default = "default_category")]
    pub category: String,
    /// signature = signature/regex; plugin = custom plugin (L7 Lua/JSON).
    #[serde(default = "default_rule_type")]
    pub rule_type: String,
    #[serde(default)]
    pub plugin_name: String,
}

fn default_rule_type() -> String {
    "signature".into()
}

fn default_phase() -> String {
    "request".into()
}
fn default_priority() -> i32 {
    100
}
fn default_true() -> bool {
    true
}
fn default_action() -> String {
    "block".into()
}
fn default_severity() -> String {
    "medium".into()
}
fn default_category() -> String {
    "custom".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomRuleSet {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomRulesFile {
    #[serde(default)]
    pub sets: Vec<CustomRuleSet>,
    #[serde(default)]
    pub rules: Vec<CustomRule>,
}

fn field_content(req: &InspectRequest, field: &str) -> String {
    match field {
        "uri" => req.uri.clone(),
        "headers" => req.headers.clone(),
        "body" => req.body.clone(),
        "method" => req.method.clone(),
        "user_agent" => req.user_agent.clone(),
        "src_ip" => req.src_ip.clone(),
        "all" => format!(
            "{} {} {} {} {} {}",
            req.method, req.uri, req.headers, req.body, req.user_agent, req.src_ip
        ),
        _ => req.uri.clone(),
    }
}

fn resolve_action(rule_action: &str, global_mode: &str) -> String {
    if global_mode == "detect" {
        return "log".into();
    }
    if rule_action == "off" {
        return "allow".into();
    }
    rule_action.to_string()
}

fn rule_matches(rule: &CustomRule, content: &str) -> bool {
    match rule.match_op.as_str() {
        "contains" => content
            .to_lowercase()
            .contains(&rule.match_pattern.to_lowercase()),
        "eq" => content == rule.match_pattern,
        // Legacy behavior: unknown match_op falls back to regex.
        _ => Regex::new(&rule.match_pattern)
            .map(|re| re.is_match(content))
            .unwrap_or(false),
    }
}

/// Evaluate custom rules by priority (request phase only).
pub fn inspect_custom_rules(
    rules: &[CustomRule],
    req: &InspectRequest,
    global_mode: &str,
) -> Option<InspectResult> {
    let mut sorted: Vec<&CustomRule> = rules.iter().filter(|r| r.enabled).collect();
    sorted.sort_by_key(|r| r.priority);

    for rule in sorted {
        if rule.phase != "request" || rule.rule_type == "plugin" {
            continue;
        }
        let content = field_content(req, &rule.match_field);
        if !rule_matches(rule, &content) {
            continue;
        }
        let action = resolve_action(&rule.action, global_mode);
        let blocked = action == "block";
        return Some(InspectResult {
            blocked,
            action,
            category: rule.category.clone(),
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            severity: rule.severity.clone(),
            matched_field: rule.match_field.clone(),
            payload_sample: crate::engine::truncate(&content, 200),
            plugin_name: Some("custom-rules".into()),
            plugin_type: Some("json".into()),
            param_path: None,
            decode_chain: None,
            semantic_score: None,
            engines: None,
        });
    }
    None
}
