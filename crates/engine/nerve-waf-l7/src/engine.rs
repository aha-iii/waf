use crate::rules::CompiledRule;
use crate::scan_targets::RequestSamples;
use nerve_waf_sdk::ProtectionModules;
use serde::Serialize;

/// ModSecurity-style chain: each step AND; `on_matched` continues on prior hit sample.
fn match_rule_chain(rule: &CompiledRule, samples: &RequestSamples) -> Option<(String, String)> {
    let mut matched_value: Option<String> = None;
    let mut first_field: Option<String> = None;
    let mut first_sample: Option<String> = None;

    for (idx, step) in rule.steps.iter().enumerate() {
        if step.on_matched {
            let prev = matched_value.as_deref()?;
            if !step.re.is_match(prev) {
                return None;
            }
            continue;
        }
        let mut hit: Option<(String, String)> = None;
        for (field_name, content) in samples.for_targets(&step.scan_targets) {
            if content.is_empty() {
                continue;
            }
            if step.re.is_match(content) {
                hit = Some((field_name.to_string(), content.to_string()));
                break;
            }
        }
        let (field, value) = hit?;
        matched_value = Some(value.clone());
        if idx == 0 {
            first_field = Some(field);
            first_sample = Some(value);
        }
    }

    Some((first_field.unwrap_or_else(|| "chain".into()), first_sample?))
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectRequest {
    pub method: String,
    pub host: String,
    pub uri: String,
    pub headers: String,
    pub body: String,
    pub src_ip: String,
    pub user_agent: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectResult {
    pub blocked: bool,
    pub action: String,
    pub category: String,
    pub rule_id: String,
    pub rule_name: String,
    pub severity: String,
    pub matched_field: String,
    pub payload_sample: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_type: Option<String>,
    /// Semantic pipeline: param paths like query.id / body.field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_path: Option<String>,
    /// Decode-chain op:depth sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_chain: Option<Vec<String>>,
    /// Fusion risk score 0–100.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engines: Option<Vec<String>>,
}

pub struct L7Engine {
    modules: ProtectionModules,
    rules: Vec<CompiledRule>,
}

impl L7Engine {
    pub fn new(modules: ProtectionModules) -> Self {
        Self::from_rules(modules, crate::embedded_rules())
    }

    pub fn from_rules(modules: ProtectionModules, rules: Vec<CompiledRule>) -> Self {
        Self { modules, rules }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn embedded_rule_count() -> usize {
        crate::embedded_rules().len()
    }

    pub fn modules(&self) -> &ProtectionModules {
        &self.modules
    }

    pub fn set_modules(&mut self, modules: ProtectionModules) {
        self.modules = modules;
    }

    pub fn set_rules(&mut self, rules: Vec<CompiledRule>) {
        self.rules = rules;
    }

    pub fn rules(&self) -> &[CompiledRule] {
        &self.rules
    }

    pub fn inspect(&self, req: &InspectRequest) -> Option<InspectResult> {
        self.inspect_with_meta(req, None, None)
    }

    pub fn inspect_with_meta(
        &self,
        req: &InspectRequest,
        plugin_name: Option<&str>,
        plugin_type: Option<&str>,
    ) -> Option<InspectResult> {
        let samples = RequestSamples::from(req);

        for rule in &self.rules {
            if !self.module_enabled(&rule.category) {
                continue;
            }
            if self.modules.is_rule_disabled(&rule.category, &rule.id) {
                continue;
            }
            if !self.rule_matches_sensitivity(rule) {
                continue;
            }
            if let Some((field_name, sample)) = match_rule_chain(rule, &samples) {
                let action = self.resolve_action(&rule.category);
                let blocked = action == "block";
                return Some(InspectResult {
                    blocked,
                    action,
                    category: rule.category.clone(),
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity.clone(),
                    matched_field: field_name,
                    payload_sample: truncate(&sample, 200),
                    plugin_name: plugin_name.map(str::to_string),
                    plugin_type: plugin_type.map(str::to_string),
                    param_path: None,
                    decode_chain: None,
                    semantic_score: None,
                    engines: None,
                });
            }
        }
        None
    }

    /// Malicious upload detect: dangerous / double extensions.
    pub fn inspect_upload(&self, req: &InspectRequest) -> Option<InspectResult> {
        if !self.module_enabled("file_upload") {
            return None;
        }
        let hay = format!("{} {} {}", req.uri, req.headers, req.body);
        let bad = [
            ".php",
            ".jsp",
            ".asp",
            ".aspx",
            ".exe",
            ".sh",
            ".bat",
            ".php.",
            ".jpg.php",
            "content-disposition: form-data",
        ];
        for sig in bad {
            if hay.to_lowercase().contains(sig) {
                return Some(InspectResult {
                    blocked: true,
                    action: self.resolve_action("file_upload"),
                    category: "file_upload".into(),
                    rule_id: "A15-upload".into(),
                    rule_name: "Malicious upload pattern".into(),
                    severity: "high".into(),
                    matched_field: "body".into(),
                    payload_sample: truncate(&hay, 120),
                    plugin_name: None,
                    plugin_type: None,
                    param_path: None,
                    decode_chain: None,
                    semantic_score: None,
                    engines: None,
                });
            }
        }
        None
    }

    /// Info leak: sensitive patterns in response body.
    pub fn inspect_response(&self, body: &str) -> Option<InspectResult> {
        if !self.module_enabled("info_leak") {
            return None;
        }
        let patterns = [
            (
                "A14-stack",
                r"(?i)(stack trace|exception in thread|at \w+\.\w+\()",
            ),
            (
                "A14-key",
                r"(?i)(api[_-]?key\s*[:=]|aws_secret|private_key)",
            ),
            ("A14-sql", r"(?i)(sql syntax.*mysql|ora-\d{5})"),
        ];
        for (id, pat) in patterns {
            if let Ok(re) = regex::Regex::new(pat) {
                if re.is_match(body) {
                    return Some(InspectResult {
                        blocked: true,
                        action: "block".into(),
                        category: "info_leak".into(),
                        rule_id: id.into(),
                        rule_name: "Sensitive response content".into(),
                        severity: "medium".into(),
                        matched_field: "response_body".into(),
                        payload_sample: truncate(body, 200),
                        plugin_name: None,
                        plugin_type: None,
                        param_path: None,
                        decode_chain: None,
                        semantic_score: None,
                        engines: None,
                    });
                }
            }
        }
        None
    }

    fn rule_matches_sensitivity(&self, rule: &CompiledRule) -> bool {
        let sensitivity = self
            .modules
            .modules
            .get(&rule.category)
            .map(|m| m.sensitivity.as_str())
            .unwrap_or("medium");
        nerve_waf_sdk::rule_matches_sensitivity(rule.paranoia_level, &rule.severity, sensitivity)
    }

    fn module_enabled(&self, category: &str) -> bool {
        self.modules
            .modules
            .get(category)
            .map(|m| m.enabled)
            .unwrap_or_else(|| {
                matches!(
                    category,
                    "sqli"
                        | "xss"
                        | "path_traversal"
                        | "file_upload"
                        | "info_leak"
                        | "prompt_injection"
                )
            })
    }

    fn resolve_action(&self, category: &str) -> String {
        let global = self.modules.mode.clone();
        let module_action = self
            .modules
            .modules
            .get(category)
            .map(|m| m.action.clone())
            .unwrap_or_else(|| global.clone());

        if module_action == "off" {
            return "allow".into();
        }
        if global == "detect" {
            return "log".into();
        }
        module_action
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

impl Default for InspectRequest {
    fn default() -> Self {
        Self {
            method: "GET".into(),
            host: String::new(),
            uri: String::new(),
            headers: String::new(),
            body: String::new(),
            src_ip: String::new(),
            user_agent: String::new(),
        }
    }
}
