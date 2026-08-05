//! 白名单：按 URI / Host / 规则 ID / 源 IP / 类别 精细放行

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

/// 路径级全量放行：仅匹配 URI 前缀（可叠加 Host / 源 IP），不指定 rule_id / category 时跳过整条检测链。
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
mod tests {
    use super::*;

    #[test]
    fn whitelist_by_rule_and_uri() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "wl-1".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/health".into()],
                    rule_ids: vec!["sql-002".into()],
                    ..Default::default()
                },
            }],
        };
        let req = WhitelistRequest {
            host: "localhost",
            uri: "/api/health",
            src_ip: "1.2.3.4",
            rule_id: "sql-002",
            category: "sqli",
        };
        assert!(is_whitelisted(&cfg, &req));
    }

    #[test]
    fn inspect_bypass_by_uri_prefix() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "auth".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/auth-center/auth".into()],
                    ..Default::default()
                },
            }],
        };
        assert!(is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/auth-center/auth?sign=abc",
            "1.2.3.4"
        ));
        assert!(!is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/other",
            "1.2.3.4"
        ));
    }

    #[test]
    fn inspect_bypass_not_when_rule_id_scoped() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "partial".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/auth-center/auth".into()],
                    rule_ids: vec!["942370".into()],
                    ..Default::default()
                },
            }],
        };
        assert!(!is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/auth-center/auth",
            "1.2.3.4"
        ));
    }
}
