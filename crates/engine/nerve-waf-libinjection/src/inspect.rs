use crate::catalog::{
    effective_rule_enabled, libinjection_rule_catalog, LibInjectionRuleDef, ScanTarget,
};
use crate::LibInjectionConfig;
use libinjectionrs::{detect_sqli, detect_xss};
use nerve_waf_l7::{InspectRequest, InspectResult};
use nerve_waf_sdk::ProtectionModules;

pub fn inspect_libinjection(
    config: &LibInjectionConfig,
    modules: &ProtectionModules,
    req: &InspectRequest,
) -> Option<InspectResult> {
    if !config.enabled {
        return None;
    }

    let samples = RequestSamples::from(req);

    for rule in libinjection_rule_catalog() {
        let paranoia_max = modules
            .modules
            .get(rule.category)
            .map(|m| nerve_waf_sdk::sensitivity_paranoia_max(&m.sensitivity))
            .unwrap_or(config.paranoia_max);
        if !effective_rule_enabled(config, rule, paranoia_max) {
            continue;
        }
        if !module_enabled(modules, rule.category) {
            continue;
        }
        if modules.is_rule_disabled(rule.category, rule.id) {
            continue;
        }
        for (field, value) in samples.for_targets(rule.scan_targets) {
            if value.is_empty() {
                continue;
            }
            if detect_value(rule, value.as_bytes()) {
                let action = resolve_action(modules, rule.category);
                let blocked = action == "block";
                return Some(InspectResult {
                    blocked,
                    action,
                    category: rule.category.into(),
                    rule_id: rule.id.into(),
                    rule_name: rule.name.into(),
                    severity: rule.severity.into(),
                    matched_field: field.into(),
                    payload_sample: truncate(value, 200),
                    plugin_name: Some("libinjection".into()),
                    plugin_type: Some("rust".into()),
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

fn detect_value(rule: &LibInjectionRuleDef, value: &[u8]) -> bool {
    if rule.detect_xss {
        detect_xss(value).is_injection()
    } else {
        detect_sqli(value).is_injection()
    }
}

fn module_enabled(modules: &ProtectionModules, category: &str) -> bool {
    modules
        .modules
        .get(category)
        .map(|m| m.enabled)
        .unwrap_or(matches!(category, "sqli" | "xss"))
}

fn resolve_action(modules: &ProtectionModules, category: &str) -> String {
    let global = modules.mode.clone();
    let module_action = modules
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

struct RequestSamples {
    uri: String,
    uri_decoded: String,
    query_values: Vec<String>,
    query_names: Vec<String>,
    body_form_values: Vec<String>,
    body: String,
    body_decoded: String,
    user_agent: String,
    referer: String,
    cookies: Vec<String>,
    cookie_names: Vec<String>,
    filename: String,
    basename: String,
}

impl RequestSamples {
    fn from(req: &InspectRequest) -> Self {
        let uri_decoded = decode_input(&req.uri);
        let body_decoded = decode_input(&req.body);
        let (query_names, query_values) = parse_query(&req.uri);
        let (_, body_form_values) = parse_form_values(&req.body);
        let (cookie_names, cookies) = parse_cookies(&req.headers);
        let referer = header_value(&req.headers, "referer");
        let filename = path_from_uri(&req.uri);
        let basename = filename
            .rsplit('/')
            .next()
            .unwrap_or(&filename)
            .to_string();
        Self {
            uri: req.uri.clone(),
            uri_decoded,
            query_values,
            query_names,
            body_form_values,
            body: req.body.clone(),
            body_decoded,
            user_agent: req.user_agent.clone(),
            referer,
            cookies,
            cookie_names,
            filename,
            basename,
        }
    }

    fn for_targets<'a>(
        &'a self,
        targets: &'static [ScanTarget],
    ) -> impl Iterator<Item = (&'static str, &'a str)> + 'a {
        targets.iter().flat_map(|t| self.values_for(*t))
    }

    fn values_for<'a>(&'a self, target: ScanTarget) -> Vec<(&'static str, &'a str)> {
        let mut out = Vec::new();
        match target {
            ScanTarget::Args => {
                out.push(("ARGS(uri)", self.uri.as_str()));
                out.push(("ARGS(uri_decoded)", self.uri_decoded.as_str()));
                for v in &self.query_values {
                    out.push(("ARGS", v.as_str()));
                }
                for v in &self.body_form_values {
                    out.push(("ARGS(body)", v.as_str()));
                }
            }
            ScanTarget::ArgsNames => {
                for n in &self.query_names {
                    out.push(("ARGS_NAMES", n.as_str()));
                }
            }
            ScanTarget::Cookies => {
                for c in &self.cookies {
                    out.push(("COOKIES", c.as_str()));
                }
            }
            ScanTarget::CookieNames => {
                for n in &self.cookie_names {
                    out.push(("COOKIE_NAMES", n.as_str()));
                }
            }
            ScanTarget::UserAgent => {
                out.push(("User-Agent", self.user_agent.as_str()));
            }
            ScanTarget::Referer => {
                out.push(("Referer", self.referer.as_str()));
            }
            ScanTarget::RequestFilename => {
                out.push(("REQUEST_FILENAME", self.filename.as_str()));
            }
            ScanTarget::RequestBasename => {
                out.push(("REQUEST_BASENAME", self.basename.as_str()));
            }
            ScanTarget::Body => {
                out.push(("BODY", self.body.as_str()));
                out.push(("BODY(decoded)", self.body_decoded.as_str()));
            }
        }
        out
    }
}

fn decode_input(s: &str) -> String {
    let plus = s.replace('+', " ");
    urlencoding::decode(&plus)
        .map(|c| c.into_owned())
        .unwrap_or(plus)
}

fn parse_form_values(body: &str) -> (Vec<String>, Vec<String>) {
    if body.is_empty() || body.starts_with('{') {
        return (Vec::new(), Vec::new());
    }
    parse_query(&format!("?{body}"))
}

fn parse_query(uri: &str) -> (Vec<String>, Vec<String>) {
    let q = uri.split('?').nth(1).unwrap_or("");
    let mut names = Vec::new();
    let mut values = Vec::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                names.push(decode_input(k));
                values.push(decode_input(v));
            }
            None => {
                names.push(decode_input(pair));
                values.push(String::new());
            }
        }
    }
    (names, values)
}

fn parse_cookies(headers: &str) -> (Vec<String>, Vec<String>) {
    let mut names = Vec::new();
    let mut values = Vec::new();
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("cookie:") {
            continue;
        }
        let rest = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
        for part in rest.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.split_once('=') {
                Some((k, v)) => {
                    names.push(k.trim().into());
                    values.push(decode_input(v.trim()));
                }
                None => names.push(part.into()),
            }
        }
    }
    (names, values)
}

fn header_value(headers: &str, name: &str) -> String {
    let needle = format!("{name}:");
    for line in headers.lines() {
        if line.len() >= needle.len() && line[..needle.len()].eq_ignore_ascii_case(&needle) {
            return line[needle.len()..].trim().into();
        }
    }
    String::new()
}

fn path_from_uri(uri: &str) -> String {
    let path = uri.split('?').next().unwrap_or(uri);
    if path.is_empty() {
        "/".into()
    } else {
        path.into()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nerve_waf_sdk::ProtectionModules;

    #[test]
    fn detects_sqli_in_query() {
        let cfg = LibInjectionConfig::default();
        let modules = ProtectionModules::default();
        let hit = inspect_libinjection(
            &cfg,
            &modules,
            &InspectRequest {
                uri: "/?id=1' OR '1'='1".into(),
                ..Default::default()
            },
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().rule_id, "942100");
    }
}
