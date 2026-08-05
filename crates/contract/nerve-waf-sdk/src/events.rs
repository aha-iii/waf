use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WafAction {
    Allow,
    Block,
    Log,
    Challenge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WafEventRecord {
    pub id: String,
    pub ts: DateTime<Utc>,
    #[serde(default = "default_site")]
    pub site_id: String,
    pub layer: String,
    pub action: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_name: Option<String>,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// 原始请求头，入库时截断到 4KB
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
    /// 原始请求体，入库时截断到 8KB
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_sample: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// inspect 入口：ebpf / http-bypass / agent-local 等
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspect_source: Option<String>,
    /// 命中时应用绑定的防护策略
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_name: Option<String>,
    /// 流水线段：semantic / expert / core / custom / pre_access
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection_stage: Option<String>,
    /// 规则集标识：libinjection / expert-crs / l7-json / custom:{id} …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
}

fn default_site() -> String {
    "default".into()
}

fn default_severity() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventQuery {
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_id: Option<String>,
    /// l7 = WAF 攻击日志；l3 = Agent 网络观测
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// 排除的类型（如 orphan_connect）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_categories: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventQueryResult {
    pub total: u64,
    pub events: Vec<WafEventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsOverview {
    pub total_events: u64,
    pub blocked: u64,
    pub logged: u64,
    pub by_category: Vec<CategoryCount>,
    pub top_ips: Vec<IpCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryCount {
    pub category: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpCount {
    pub src_ip: String,
    pub count: u64,
}
