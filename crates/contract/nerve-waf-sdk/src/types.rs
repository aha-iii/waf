use crate::{l7_legacy_health_check_to_checks, L7UpstreamChecks};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7UpstreamNode {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_node_weight")]
    pub weight: u32,
}

fn default_node_weight() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7Upstream {
    pub id: String,
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_kind: Option<String>,
    #[serde(default = "default_upstream_scheme")]
    pub scheme: String,
    #[serde(default = "default_upstream_type")]
    #[serde(rename = "type")]
    pub upstream_type: String,
    #[serde(default)]
    pub nodes: Vec<L7UpstreamNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<L7UpstreamTransport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<L7UpstreamChecks>,
    /// Legacy field; migrated into checks by normalize_checks on load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check: Option<L7HealthCheck>,
}

fn default_upstream_scheme() -> String {
    "http".into()
}

fn default_upstream_type() -> String {
    "roundrobin".into()
}

fn default_health_path() -> String {
    "/".into()
}

fn default_hc_interval() -> u32 {
    10
}

fn default_hc_type() -> String {
    "http".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct L7UpstreamTransport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_capacity: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_idle_timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_max_requests: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L7HealthCheck {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_hc_type")]
    pub hc_type: String,
    #[serde(default = "default_health_path")]
    pub path: String,
    #[serde(default = "default_hc_interval")]
    pub interval_sec: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_sec: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthy_successes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unhealthy_timeouts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub healthy_statuses: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unhealthy_statuses: Option<Vec<u16>>,
}

impl Default for L7HealthCheck {
    fn default() -> Self {
        Self {
            enabled: false,
            hc_type: default_hc_type(),
            path: default_health_path(),
            interval_sec: default_hc_interval(),
            timeout_sec: None,
            healthy_successes: None,
            unhealthy_timeouts: None,
            healthy_statuses: None,
            unhealthy_statuses: None,
        }
    }
}

impl L7Upstream {
    pub fn is_gateway_origin(&self) -> bool {
        self.origin_kind
            .as_deref()
            .map(|k| k.eq_ignore_ascii_case("gateway"))
            .unwrap_or(false)
    }

    pub fn normalize_checks(&mut self) {
        if self.checks.is_some() {
            self.health_check = None;
            return;
        }
        if let Some(hc) = self.health_check.take() {
            if hc.enabled {
                self.checks = Some(l7_legacy_health_check_to_checks(
                    true,
                    &hc.hc_type,
                    &hc.path,
                    hc.interval_sec,
                    hc.timeout_sec,
                    hc.healthy_successes,
                    hc.unhealthy_timeouts,
                    hc.healthy_statuses,
                ));
            }
        }
    }

    pub fn pick_url(&self, rr_index: usize) -> Option<String> {
        if !self.enabled || self.nodes.is_empty() {
            return None;
        }
        let enabled: Vec<&L7UpstreamNode> = self.nodes.iter().filter(|n| n.weight > 0).collect();
        let node = if enabled.is_empty() {
            &self.nodes[rr_index % self.nodes.len()]
        } else {
            enabled[rr_index % enabled.len()]
        };
        Some(format!("{}://{}:{}", self.scheme, node.host, node.port))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct L7Match {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri_regex: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RouteMatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod: Option<String>,
    /// L7 HTTP match (config reserved; dataplane today is mainly L3/L4 eBPF).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l7: Option<L7Match>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutePluginRef {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_enabled() -> bool {
    true
}

fn default_layer() -> String {
    "l3".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WafRoute {
    pub id: String,
    pub name: String,
    pub priority: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Route layer: `l3` network policy / `l7` HTTP route.
    #[serde(default = "default_layer")]
    pub layer: String,
    #[serde(default, rename = "match")]
    pub match_: RouteMatch,
    #[serde(default)]
    pub plugins: Vec<RoutePluginRef>,
    /// Upstream pool bound to an L7 route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_id: Option<String>,
}

impl WafRoute {
    pub fn route_match(&self) -> &RouteMatch {
        &self.match_
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WafConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub routes: Vec<WafRoute>,
}

fn default_version() -> u32 {
    1
}

impl Default for WafConfig {
    fn default() -> Self {
        Self {
            version: 1,
            routes: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginAction {
    DenyIp {
        ip: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    AllowIp {
        ip: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    BaselineAllow {
        pod: String,
        ip: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Log {
        level: LogLevel,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub actions: Vec<PluginAction>,
    pub routes_applied: usize,
    pub plugins_applied: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub l7_bundle: Option<crate::L7Bundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub ok: bool,
    pub plan: SyncPlan,
    pub applied: usize,
    pub errors: Vec<String>,
}
