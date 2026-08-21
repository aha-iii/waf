use crate::LibInjectionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScanTarget {
    Args,
    ArgsNames,
    Cookies,
    CookieNames,
    UserAgent,
    Referer,
    RequestFilename,
    RequestBasename,
    Body,
}

impl ScanTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Args => "ARGS",
            Self::ArgsNames => "ARGS_NAMES",
            Self::Cookies => "COOKIES",
            Self::CookieNames => "COOKIE_NAMES",
            Self::UserAgent => "User-Agent",
            Self::Referer => "Referer",
            Self::RequestFilename => "REQUEST_FILENAME",
            Self::RequestBasename => "REQUEST_BASENAME",
            Self::Body => "BODY",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LibInjectionRuleDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub paranoia_level: u8,
    pub severity: &'static str,
    pub crs_operator: &'static str,
    pub scan_targets: &'static [ScanTarget],
    pub detect_xss: bool,
}

/// Built-in injection rule catalog (aligned with core `@detectSQLi` / `@detectXSS`).
pub fn libinjection_rule_catalog() -> &'static [LibInjectionRuleDef] {
    use ScanTarget::*;
    &[
        LibInjectionRuleDef {
            id: "942100",
            name: "SQL Injection Attack · 参数/Cookie/UA",
            category: "sqli",
            paranoia_level: 1,
            severity: "critical",
            crs_operator: "@detectSQLi",
            scan_targets: &[Cookies, CookieNames, UserAgent, Referer, Args, ArgsNames, Body],
            detect_xss: false,
        },
        LibInjectionRuleDef {
            id: "942101",
            name: "SQL Injection Attack · 路径/文件名",
            category: "sqli",
            paranoia_level: 2,
            severity: "critical",
            crs_operator: "@detectSQLi",
            scan_targets: &[RequestBasename, RequestFilename],
            detect_xss: false,
        },
        LibInjectionRuleDef {
            id: "941100",
            name: "XSS Attack · 参数/Cookie/UA",
            category: "xss",
            paranoia_level: 1,
            severity: "critical",
            crs_operator: "@detectXSS",
            scan_targets: &[Cookies, CookieNames, UserAgent, Args, ArgsNames, Body],
            detect_xss: true,
        },
        LibInjectionRuleDef {
            id: "941101",
            name: "XSS Attack · 路径/Referer",
            category: "xss",
            paranoia_level: 2,
            severity: "critical",
            crs_operator: "@detectXSS",
            scan_targets: &[RequestFilename, Referer],
            detect_xss: true,
        },
    ]
}

pub fn effective_rule_enabled(
    config: &LibInjectionConfig,
    rule: &LibInjectionRuleDef,
    paranoia_max: u8,
) -> bool {
    if let Some(sw) = config.rules.get(rule.id) {
        if let Some(enabled) = sw.enabled {
            return enabled;
        }
    }
    rule.paranoia_level <= paranoia_max
}
