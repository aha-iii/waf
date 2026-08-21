//! Upstream health checks (aligned with APISIX / lua-resty-healthcheck `checks`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct L7UpstreamChecks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<L7ActiveHealthCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive: Option<L7PassiveHealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7ActiveHealthCheck {
    #[serde(default = "default_hc_http")]
    #[serde(rename = "type")]
    pub check_type: String,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_http_path")]
    pub http_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default = "default_true")]
    pub https_verify_certificate: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub req_headers: Vec<String>,
    #[serde(default)]
    pub healthy: L7ActiveHealthyThreshold,
    #[serde(default)]
    pub unhealthy: L7ActiveUnhealthyThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7PassiveHealthCheck {
    #[serde(default = "default_hc_http")]
    #[serde(rename = "type")]
    pub check_type: String,
    #[serde(default)]
    pub healthy: L7PassiveHealthyThreshold,
    #[serde(default)]
    pub unhealthy: L7PassiveUnhealthyThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7ActiveHealthyThreshold {
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(default = "default_active_successes")]
    pub successes: u32,
    #[serde(default = "default_active_healthy_statuses")]
    pub http_statuses: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7ActiveUnhealthyThreshold {
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(default = "default_active_http_failures")]
    pub http_failures: u32,
    #[serde(default = "default_active_tcp_failures")]
    pub tcp_failures: u32,
    #[serde(default = "default_active_timeouts")]
    pub timeouts: u32,
    #[serde(default = "default_active_unhealthy_statuses")]
    pub http_statuses: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7PassiveHealthyThreshold {
    #[serde(default = "default_passive_successes")]
    pub successes: u32,
    #[serde(default = "default_passive_healthy_statuses")]
    pub http_statuses: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7PassiveUnhealthyThreshold {
    #[serde(default = "default_passive_http_failures")]
    pub http_failures: u32,
    #[serde(default = "default_passive_tcp_failures")]
    pub tcp_failures: u32,
    #[serde(default = "default_passive_timeouts")]
    pub timeouts: u32,
    #[serde(default = "default_passive_unhealthy_statuses")]
    pub http_statuses: Vec<u16>,
}

impl Default for L7ActiveHealthCheck {
    fn default() -> Self {
        Self {
            check_type: default_hc_http(),
            timeout: default_timeout(),
            concurrency: default_concurrency(),
            http_path: default_http_path(),
            host: None,
            port: None,
            https_verify_certificate: true,
            req_headers: vec![],
            healthy: L7ActiveHealthyThreshold::default(),
            unhealthy: L7ActiveUnhealthyThreshold::default(),
        }
    }
}

impl Default for L7PassiveHealthCheck {
    fn default() -> Self {
        Self {
            check_type: default_hc_http(),
            healthy: L7PassiveHealthyThreshold::default(),
            unhealthy: L7PassiveUnhealthyThreshold::default(),
        }
    }
}

impl Default for L7ActiveHealthyThreshold {
    fn default() -> Self {
        Self {
            interval: default_interval(),
            successes: default_active_successes(),
            http_statuses: default_active_healthy_statuses(),
        }
    }
}

impl Default for L7ActiveUnhealthyThreshold {
    fn default() -> Self {
        Self {
            interval: default_interval(),
            http_failures: default_active_http_failures(),
            tcp_failures: default_active_tcp_failures(),
            timeouts: default_active_timeouts(),
            http_statuses: default_active_unhealthy_statuses(),
        }
    }
}

impl Default for L7PassiveHealthyThreshold {
    fn default() -> Self {
        Self {
            successes: default_passive_successes(),
            http_statuses: default_passive_healthy_statuses(),
        }
    }
}

impl Default for L7PassiveUnhealthyThreshold {
    fn default() -> Self {
        Self {
            http_failures: default_passive_http_failures(),
            tcp_failures: default_passive_tcp_failures(),
            timeouts: default_passive_timeouts(),
            http_statuses: default_passive_unhealthy_statuses(),
        }
    }
}

impl L7UpstreamChecks {
    pub fn active_enabled(&self) -> bool {
        self.active
            .as_ref()
            .map(|a| a.healthy.interval > 0 || a.unhealthy.interval > 0)
            .unwrap_or(false)
    }

    pub fn passive_enabled(&self) -> bool {
        self.passive.is_some()
    }
}

pub fn l7_legacy_health_check_to_checks(
    enabled: bool,
    hc_type: &str,
    path: &str,
    interval_sec: u32,
    timeout_sec: Option<u32>,
    healthy_successes: Option<u32>,
    unhealthy_timeouts: Option<u32>,
    healthy_statuses: Option<Vec<u16>>,
) -> L7UpstreamChecks {
    if !enabled {
        return L7UpstreamChecks::default();
    }
    L7UpstreamChecks {
        active: Some(L7ActiveHealthCheck {
            check_type: if hc_type.is_empty() {
                default_hc_http()
            } else {
                hc_type.into()
            },
            timeout: timeout_sec.unwrap_or(1),
            http_path: if path.is_empty() {
                default_http_path()
            } else {
                path.into()
            },
            healthy: L7ActiveHealthyThreshold {
                interval: interval_sec.max(1),
                successes: healthy_successes.unwrap_or(2),
                http_statuses: healthy_statuses.unwrap_or_else(default_active_healthy_statuses),
            },
            unhealthy: L7ActiveUnhealthyThreshold {
                timeouts: unhealthy_timeouts.unwrap_or(3),
                ..Default::default()
            },
            ..Default::default()
        }),
        passive: Some(L7PassiveHealthCheck::default()),
    }
}

fn default_hc_http() -> String {
    "http".into()
}
fn default_timeout() -> u32 {
    1
}
fn default_concurrency() -> u32 {
    10
}
fn default_http_path() -> String {
    "/".into()
}
fn default_interval() -> u32 {
    1
}
fn default_active_successes() -> u32 {
    2
}
fn default_active_http_failures() -> u32 {
    5
}
fn default_active_tcp_failures() -> u32 {
    2
}
fn default_active_timeouts() -> u32 {
    3
}
fn default_passive_successes() -> u32 {
    5
}
fn default_passive_http_failures() -> u32 {
    3
}
fn default_passive_tcp_failures() -> u32 {
    2
}
fn default_passive_timeouts() -> u32 {
    7
}
fn default_true() -> bool {
    true
}

fn default_active_healthy_statuses() -> Vec<u16> {
    vec![200, 302]
}

fn default_active_unhealthy_statuses() -> Vec<u16> {
    vec![429, 404, 500, 501, 502, 503, 504, 505]
}

fn default_passive_healthy_statuses() -> Vec<u16> {
    vec![
        200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306,
        307, 308,
    ]
}

fn default_passive_unhealthy_statuses() -> Vec<u16> {
    vec![429, 500, 503]
}
