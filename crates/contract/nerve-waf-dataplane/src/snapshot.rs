use nerve_waf_api_sec::ApiSecConfig;
use nerve_waf_behavior::BehaviorConfig;
use nerve_waf_bot::BotConfig;
use nerve_waf_cc::CcConfig;
use nerve_waf_geo::GeoConfig;
use nerve_waf_intel::IntelEntry;
use nerve_waf_l7::{CustomRule, RawRule};
use nerve_waf_libinjection::LibInjectionConfig;
use nerve_waf_sdk::{L7Upstream, ProtectionModules};
use nerve_waf_semantic::SemanticConfig;
use nerve_waf_store::AppCrsBinding;
use nerve_waf_whitelist::WhitelistConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeRoute {
    pub id: String,
    pub site_id: String,
    pub host: String,
    pub upstream_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeSiteMeta {
    pub id: String,
    pub name: String,
    pub hosts: Vec<String>,
    pub ingress_mode: String,
    pub upstream_id: String,
}

/// 与 API `execute_inspect_traced` 对齐的 per-site 检测输入。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SiteInspectSnapshot {
    pub modules: ProtectionModules,
    pub semantic: SemanticConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_binding: Option<AppCrsBinding>,
    /// 可序列化 L7 规则（Edge 侧 compile 为 L7Engine）。
    #[serde(default)]
    pub l7_rules: Vec<RawRule>,
    #[serde(default)]
    pub custom_rules: Vec<CustomRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_name: Option<String>,
}

/// 全局平台 overlay（所有 site 共享）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalInspectOverlay {
    #[serde(default)]
    pub whitelist: WhitelistConfig,
    #[serde(default)]
    pub cc: CcConfig,
    #[serde(default)]
    pub bot: BotConfig,
    #[serde(default)]
    pub libinjection: LibInjectionConfig,
    #[serde(default)]
    pub geo: GeoConfig,
    #[serde(default)]
    pub intel_entries: Vec<IntelEntry>,
    #[serde(default)]
    pub api_sec: ApiSecConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WafEdgeSnapshot {
    pub generation: u64,
    pub checksum: String,
    pub built_at: String,
    pub routes: Vec<EdgeRoute>,
    pub upstreams: Vec<L7Upstream>,
    pub sites: Vec<EdgeSiteMeta>,
    pub site_inspect: HashMap<String, SiteInspectSnapshot>,
    pub global: GlobalInspectOverlay,
}

impl Default for WafEdgeSnapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            checksum: String::new(),
            built_at: chrono::Utc::now().to_rfc3339(),
            routes: Vec::new(),
            upstreams: Vec::new(),
            sites: Vec::new(),
            site_inspect: HashMap::new(),
            global: GlobalInspectOverlay::default(),
        }
    }
}
