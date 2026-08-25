//! Whitelist / false-positive tuning by URI, host, rule ID, source IP, or category.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhitelistConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<WhitelistRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistRule {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub match_on: WhitelistMatch,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WhitelistMatch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uri_prefixes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub src_ips: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WhitelistRequest<'a> {
    pub host: &'a str,
    pub uri: &'a str,
    pub src_ip: &'a str,
    pub rule_id: &'a str,
    pub category: &'a str,
}

fn default_true() -> bool {
    true
}

pub fn is_whitelisted(cfg: &WhitelistConfig, req: &WhitelistRequest<'_>) -> bool {
    if !cfg.enabled {
        return false;
    }
    cfg.rules.iter().any(|r| r.enabled && rule_matches(r, req))
}

/// Path-level full allow: match URI prefix only (optional Host / src IP); without rule_id / category, skip entire detect chain.
pub fn is_inspect_bypassed(cfg: &WhitelistConfig, host: &str, uri: &str, src_ip: &str) -> bool {
    if !cfg.enabled {
        return false;
    }
    let path = uri.split('?').next().unwrap_or(uri);
    let req = WhitelistRequest {
        host,
        uri: path,
        src_ip,
        rule_id: "",
        category: "",
    };
    cfg.rules.iter().any(|r| {
        if !r.enabled {
            return false;
        }
        let m = &r.match_on;
        if m.uri_prefixes.is_empty() || !m.rule_ids.is_empty() || !m.categories.is_empty() {
            return false;
        }
        rule_matches(r, &req)
    })
}

fn rule_matches(rule: &WhitelistRule, req: &WhitelistRequest<'_>) -> bool {
    let m = &rule.match_on;
    if !m.hosts.is_empty()
        && !m.hosts.iter().any(|h| h == "*" || h.eq_ignore_ascii_case(req.host))
    {
        return false;
    }
    if !m.uri_prefixes.is_empty()
        && !m
            .uri_prefixes
            .iter()
            .any(|p| req.uri.starts_with(p) || req.uri.starts_with(&format!("/{p}")))
    {
        return false;
    }
    if !m.rule_ids.is_empty() && !m.rule_ids.iter().any(|id| id == req.rule_id) {
        return false;
    }
    if !m.categories.is_empty()
        && !m
            .categories
            .iter()
            .any(|c| c.eq_ignore_ascii_case(req.category))
    {
        return false;
    }
    if !m.src_ips.is_empty() && !m.src_ips.iter().any(|ip| ip == req.src_ip) {
        return false;
    }
    true
}

#[cfg(test)]
mod test;
