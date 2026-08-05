//! 平台配置：CC / Geo / WAF 防护站点（文件持久化；告警见 Nerve-alert）
use nerve_waf_api_sec::ApiSecConfig;
use nerve_waf_behavior::{BehaviorConfig, BehaviorEngine};
use nerve_waf_cc::{CcConfig, CcEngine};
use nerve_waf_challenge::ChallengeStore;
use nerve_waf_bot::BotConfig;
use nerve_waf_geo::{GeoCidrRule, GeoConfig, GeoEngine};
use nerve_waf_intel::{IntelEntry, IntelFeed};
use nerve_waf_libinjection::LibInjectionConfig;
use nerve_waf_license::{LicenseInfo, UsageMeter};
use nerve_waf_l7::{embedded_rules, CompiledRule};
use nerve_waf_rbac::RbacConfig;
use nerve_waf_rules_ops::{
    crs_versions_dir, list_crs_versions, load_packs_from_dir, merge_rules_with_patches,
    replace_rules_dir,
    rollback_crs, snapshot_crs, CrsVersionRecord, LoadedPack, RulesPackMeta,
};
use nerve_waf_service_policy::ServiceAllowPolicy;
use nerve_waf_tamper::{TamperConfig, TamperGuard};
use nerve_waf_whitelist::WhitelistConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn default_integration() -> String {
    "standalone".into()
}

fn default_ingress_mode() -> String {
    "standalone".into()
}

fn default_tls_mode() -> String {
    "auto".into()
}

fn default_dns_status() -> String {
    "pending".into()
}

fn default_xff_hops() -> u32 {
    1
}

/// 回源节点（CNAME 边缘模式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressOrigin {
    pub address: String,
    #[serde(default = "default_origin_port")]
    pub port: u16,
    #[serde(default = "default_origin_scheme")]
    pub scheme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(default = "default_origin_weight")]
    pub weight: u32,
    #[serde(default)]
    pub backup: bool,
    /// tcp | http | none
    #[serde(default = "default_hc_type")]
    pub hc_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hc_path: Option<String>,
    #[serde(default = "default_hc_interval")]
    pub hc_interval_secs: u64,
}

fn default_origin_port() -> u16 {
    80
}
fn default_origin_scheme() -> String {
    "http".into()
}
fn default_origin_weight() -> u32 {
    100
}
fn default_hc_type() -> String {
    "tcp".into()
}
fn default_hc_interval() -> u64 {
    10
}

/// WAF 防护应用（域名/Host 与检测策略绑定；CNAME 模式下兼作边缘编排源）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedSite {
    pub id: String,
    pub name: String,
    pub hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    /// standalone | cname | nerve-gateway | gateway-bypass | agent | other
    #[serde(default = "default_integration")]
    pub integration: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_cert: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_key: Option<String>,
    /// standalone | cname | agent | bypass（与 integration 对齐；cname 时优先）
    #[serde(default = "default_ingress_mode")]
    pub ingress_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cname_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cname_fqdn: Option<String>,
    /// pending | checking | active | error
    #[serde(default = "default_dns_status")]
    pub dns_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_error: Option<String>,
    /// auto | upload
    #[serde(default = "default_tls_mode")]
    pub tls_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_id: Option<String>,
    #[serde(default)]
    pub websocket_enabled: bool,
    #[serde(default = "default_xff_hops")]
    pub xff_trusted_hops: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub origins: Vec<IngressOrigin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edge_pop_ids: Vec<String>,
    /// 绑定 Gateway 已有 upstream；空则按 origins 自动创建 `up-{site_id}`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway_upstream_id: Option<String>,
}

/// 校验 WAF 应用列表（Console PUT /api/sites）
pub fn validate_sites(sites: &[ProtectedSite]) -> Result<(), String> {
    if sites.is_empty() {
        return Err("至少保留一个 WAF 应用".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut host_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for s in sites {
        let id = s.id.trim();
        if id.is_empty() {
            return Err("应用 ID 不能为空".into());
        }
        if id.contains(char::is_whitespace) {
            return Err(format!("应用 ID 不能含空白: {id}"));
        }
        if !ids.insert(id.to_string()) {
            return Err(format!("重复的应用 ID: {id}"));
        }
        if s.name.trim().is_empty() {
            return Err(format!("应用 {id} 名称不能为空"));
        }
        if s.hosts.is_empty() {
            return Err(format!("应用 {id} 至少配置一个 Host"));
        }
        if !s.enabled {
            continue;
        }
        for h in &s.hosts {
            let host = h.trim().to_lowercase();
            if host.is_empty() {
                return Err(format!("应用 {id} 含空 Host"));
            }
            if host == "*" {
                continue;
            }
            if let Some(other) = host_map.insert(host.clone(), id.to_string()) {
                if other != id {
                    return Err(format!("Host `{host}` 已被应用 `{other}` 占用"));
                }
            }
        }
    }
    Ok(())
}

/// 兼容旧字段名 gateway_sites / 旧 API
pub type GatewaySite = ProtectedSite;

/// L7 高危命中后自动 eBPF 封禁源 IP（经 Nerve Agent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDenyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_min_severity")]
    pub min_severity: String,
    #[serde(default = "default_auto_deny_categories")]
    pub categories: Vec<String>,
}

fn default_min_severity() -> String {
    "high".into()
}

fn default_auto_deny_categories() -> Vec<String> {
    vec![
        "sqli".into(),
        "rce".into(),
        "webshell".into(),
        "ssrf".into(),
    ]
}

impl Default for AutoDenyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_severity: default_min_severity(),
            categories: default_auto_deny_categories(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlatformFiles {
    #[serde(default)]
    pub cc: CcConfig,
    #[serde(default)]
    pub geo: GeoConfig,
    #[serde(default)]
    pub geo_cidrs: Vec<GeoCidrRule>,
    #[serde(default, alias = "gateway_sites")]
    pub protected_sites: Vec<ProtectedSite>,
    #[serde(default)]
    pub service_policies: Vec<ServiceAllowPolicy>,
    #[serde(default)]
    pub east_west_l7: Vec<nerve_waf_np_compiler::EastWestL7Policy>,
    #[serde(default)]
    pub api_sec: ApiSecConfig,
    #[serde(default)]
    pub tamper: TamperConfig,
    #[serde(default)]
    pub rbac: RbacConfig,
    #[serde(default)]
    pub license: LicenseInfo,
    #[serde(default)]
    pub rules_dir: Option<String>,
    #[serde(default)]
    pub clusters: Vec<ClusterRecord>,
    #[serde(default)]
    pub intel_entries: Vec<IntelEntry>,
    #[serde(default)]
    pub whitelist: WhitelistConfig,
    #[serde(default)]
    pub bot: BotConfig,
    #[serde(default)]
    pub behavior: BehaviorConfig,
    #[serde(default)]
    pub libinjection: LibInjectionConfig,
    #[serde(default)]
    pub auto_deny: AutoDenyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRecord {
    pub cluster_id: String,
    #[serde(default)]
    pub relay_url: Option<String>,
    #[serde(default)]
    pub agent_count: u64,
}

pub struct PlatformServices {
    pub cc: Arc<CcEngine>,
    pub geo: std::sync::RwLock<GeoEngine>,
    pub sites: Arc<std::sync::RwLock<Vec<ProtectedSite>>>,
    service_policies: Arc<std::sync::RwLock<Vec<ServiceAllowPolicy>>>,
    east_west: Arc<std::sync::RwLock<Vec<nerve_waf_np_compiler::EastWestL7Policy>>>,
    intel: Arc<std::sync::RwLock<Vec<IntelEntry>>>,
    api_sec: Arc<std::sync::RwLock<ApiSecConfig>>,
    tamper: Arc<TamperGuard>,
    challenge: Arc<ChallengeStore>,
    rbac: Arc<std::sync::RwLock<RbacConfig>>,
    usage: Arc<UsageMeter>,
    rules_packs: Arc<std::sync::RwLock<Vec<LoadedPack>>>,
    patch_packs: Arc<std::sync::RwLock<Vec<LoadedPack>>>,
    ai_packs: Arc<std::sync::RwLock<Vec<LoadedPack>>>,
    whitelist: Arc<std::sync::RwLock<WhitelistConfig>>,
    pub bot: Arc<std::sync::RwLock<BotConfig>>,
    pub behavior: Arc<std::sync::RwLock<BehaviorEngine>>,
    libinjection: Arc<std::sync::RwLock<LibInjectionConfig>>,
    auto_deny: Arc<std::sync::RwLock<AutoDenyConfig>>,
    clusters: Arc<std::sync::RwLock<Vec<ClusterRecord>>>,
    l7_rule_count: Arc<std::sync::RwLock<usize>>,
    data_dir: PathBuf,
    rules_dir: PathBuf,
    patches_dir: PathBuf,
    ai_rules_dir: PathBuf,
    /// true：protected_sites 存 PostgreSQL，platform.json 不再写入站点列表
    sites_persist_db: bool,
}

impl PlatformServices {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("platform.json");
        let files = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            PlatformFiles::default()
        };

        let rules_dir = normalize_rules_dir(data_dir, files.rules_dir.as_deref(), "crs");
        let patches_dir = resolve_rules_dir(data_dir, "patches");
        let ai_rules_dir = resolve_rules_dir(data_dir, "ai");

        let mut rbac = files.rbac.clone();
        nerve_waf_rbac::merge_env_keys(&mut rbac);

        let crs_packs = load_packs_from_dir(&rules_dir).unwrap_or_default();
        let patch_packs = load_packs_from_dir(&patches_dir).unwrap_or_default();
        let ai = load_packs_from_dir(&ai_rules_dir).unwrap_or_default();
        let merged_count =
            merge_rules_with_patches(embedded_rules(), &crs_packs, &patch_packs).len()
                + ai.iter().map(|p| p.rules.len()).sum::<usize>();

        let geo_cidrs = load_geo_cidrs(data_dir, &files.geo_cidrs);
        let cc = Arc::new(CcEngine::new(files.cc.clone()));
        let geo = std::sync::RwLock::new(GeoEngine::new(files.geo.clone(), geo_cidrs));
        let sites = Arc::new(std::sync::RwLock::new(files.protected_sites.clone()));
        let service_policies = Arc::new(std::sync::RwLock::new(files.service_policies.clone()));
        let east_west = Arc::new(std::sync::RwLock::new(files.east_west_l7.clone()));
        let intel = Arc::new(std::sync::RwLock::new(files.intel_entries.clone()));

        Self {
            cc,
            geo,
            sites,
            service_policies,
            east_west,
            intel,
            api_sec: Arc::new(std::sync::RwLock::new(files.api_sec.clone())),
            tamper: Arc::new(TamperGuard::new(files.tamper.clone())),
            challenge: Arc::new(ChallengeStore::default()),
            rbac: Arc::new(std::sync::RwLock::new(rbac)),
            usage: Arc::new(UsageMeter::new(files.license.clone())),
            rules_packs: Arc::new(std::sync::RwLock::new(crs_packs)),
            patch_packs: Arc::new(std::sync::RwLock::new(patch_packs)),
            ai_packs: Arc::new(std::sync::RwLock::new(ai.clone())),
            whitelist: Arc::new(std::sync::RwLock::new(files.whitelist.clone())),
            bot: Arc::new(std::sync::RwLock::new(files.bot.clone())),
            behavior: Arc::new(std::sync::RwLock::new(BehaviorEngine::new(
                files.behavior.clone(),
            ))),
            libinjection: Arc::new(std::sync::RwLock::new(files.libinjection.clone())),
            auto_deny: Arc::new(std::sync::RwLock::new(files.auto_deny.clone())),
            clusters: Arc::new(std::sync::RwLock::new(files.clusters.clone())),
            l7_rule_count: Arc::new(std::sync::RwLock::new(merged_count)),
            data_dir: data_dir.to_path_buf(),
            rules_dir,
            patches_dir,
            ai_rules_dir,
            sites_persist_db: false,
        }
    }

    pub fn set_sites_persist_db(&mut self, yes: bool) {
        self.sites_persist_db = yes;
    }

    pub fn sites_persist_db(&self) -> bool {
        self.sites_persist_db
    }

    pub fn set_sites(&self, sites: Vec<ProtectedSite>) {
        if let Ok(mut guard) = self.sites.write() {
            *guard = sites;
        }
    }

    pub fn file_sites_snapshot(&self) -> Vec<ProtectedSite> {
        self.sites.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let geo_cfg = self.geo.read().map_err(|e| e.to_string())?.config().clone();
        let geo_cidrs = self.geo_cidrs_snapshot();
        let sites = self.sites.read().map_err(|e| e.to_string())?.clone();
        let service_policies = self.service_policies.read().map_err(|e| e.to_string())?.clone();
        let east_west_l7 = self.east_west.read().map_err(|e| e.to_string())?.clone();
        let files = PlatformFiles {
            cc: self.cc.config(),
            geo: geo_cfg,
            geo_cidrs,
            protected_sites: if self.sites_persist_db {
                vec![]
            } else {
                sites
            },
            service_policies,
            east_west_l7,
            api_sec: self.api_sec_config(),
            tamper: self.tamper_config(),
            rbac: self.rbac_config(),
            license: self.usage.snapshot().license,
            rules_dir: Some(self.rules_dir.to_string_lossy().into()),
            clusters: self.clusters.read().map_err(|e| e.to_string())?.clone(),
            intel_entries: self.intel_entries(),
            whitelist: self.whitelist_config(),
            bot: self.bot_config(),
            behavior: self.behavior_config(),
            libinjection: self.libinjection_config(),
            auto_deny: self.auto_deny_config(),
        };
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let data = serde_json::to_string_pretty(&files).map_err(|e| e.to_string())?;
        std::fs::write(self.data_dir.join("platform.json"), data).map_err(|e| e.to_string())
    }

    pub fn resolve_site(&self, host: &str) -> Option<GatewaySite> {
        let sites = self.sites.read().ok()?;
        let host_lc = host.trim().to_lowercase();
        sites
            .iter()
            .find(|s| {
                s.enabled
                    && s.hosts.iter().any(|h| {
                        let pat = h.trim().to_lowercase();
                        pat == host_lc || pat == "*"
                    })
            })
            .cloned()
            .or_else(|| sites.iter().find(|s| s.enabled && s.id == "default").cloned())
    }

    /// 旁路 /api/inspect：优先 app_id；未传或未知则使用系统默认 `default`。
    pub fn resolve_inspect_site_id(&self, app_id: Option<&str>) -> String {
        let Some(id) = app_id.map(str::trim).filter(|s| !s.is_empty()) else {
            return "default".into();
        };
        let sites = self.list_sites();
        if sites.iter().any(|s| s.id == id) {
            id.to_string()
        } else {
            tracing::warn!("inspect: unknown app_id={id}, fallback to default");
            "default".into()
        }
    }

    pub fn service_policies(&self) -> Vec<ServiceAllowPolicy> {
        self.service_policies.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn set_service_policies(&self, policies: Vec<ServiceAllowPolicy>) -> Result<(), String> {
        for p in &policies {
            nerve_waf_service_policy::validate_policy(p).map_err(|e| e.to_string())?;
        }
        *self.service_policies.write().map_err(|e| e.to_string())? = policies;
        self.save()
    }

    pub fn list_sites(&self) -> Vec<GatewaySite> {
        self.sites.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn east_west_policies(&self) -> Vec<nerve_waf_np_compiler::EastWestL7Policy> {
        self.east_west.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn set_east_west_policies(
        &self,
        policies: Vec<nerve_waf_np_compiler::EastWestL7Policy>,
    ) -> Result<(), String> {
        *self.east_west.write().map_err(|e| e.to_string())? = policies;
        self.save()
    }

    pub fn intel_entries(&self) -> Vec<IntelEntry> {
        self.intel.read().map(|e| e.clone()).unwrap_or_default()
    }

    pub fn intel_feed(&self) -> IntelFeed {
        IntelFeed::from_entries(self.intel_entries())
    }

    pub fn set_intel_feed(&self, entries: Vec<IntelEntry>) -> Result<(), String> {
        *self.intel.write().map_err(|e| e.to_string())? = entries;
        self.save()
    }

    pub fn whitelist_config(&self) -> WhitelistConfig {
        self.whitelist.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_whitelist_config(&self, cfg: WhitelistConfig) -> Result<(), String> {
        *self.whitelist.write().map_err(|e| e.to_string())? = cfg;
        self.save()
    }

    pub fn bot_config(&self) -> BotConfig {
        self.bot.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_bot_config(&self, cfg: BotConfig) -> Result<(), String> {
        *self.bot.write().map_err(|e| e.to_string())? = cfg;
        self.save()
    }

    /// Console 防护模块同步到 CC / Bot 平台配置（含模块级阈值覆盖）
    pub fn sync_from_protection_modules(&self, modules: &nerve_waf_sdk::ProtectionModules) {
        let cc = modules.merge_cc_config(&self.cc.config());
        self.cc.update_config(cc);
        let bot = modules.merge_bot_config(&self.bot_config());
        let _ = self.set_bot_config(bot);
    }

    pub fn behavior_config(&self) -> BehaviorConfig {
        self.behavior
            .read()
            .map(|e| e.config().clone())
            .unwrap_or_default()
    }

    pub fn set_behavior_config(&self, cfg: BehaviorConfig) -> Result<(), String> {
        self.behavior
            .write()
            .map_err(|e| e.to_string())?
            .update_config(cfg);
        self.save()
    }

    pub fn libinjection_config(&self) -> LibInjectionConfig {
        self.libinjection
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    pub fn set_libinjection_config(&self, cfg: LibInjectionConfig) -> Result<(), String> {
        let paranoia = cfg.paranoia_max.clamp(1, 4);
        let mut next = cfg;
        next.paranoia_max = paranoia;
        *self.libinjection.write().map_err(|e| e.to_string())? = next;
        self.save()
    }

    pub fn libinjection_status(&self) -> nerve_waf_libinjection::LibInjectionStatus {
        self.libinjection_config().status()
    }

    pub fn auto_deny_config(&self) -> AutoDenyConfig {
        self.auto_deny.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_auto_deny_config(&self, cfg: AutoDenyConfig) -> Result<(), String> {
        *self.auto_deny.write().map_err(|e| e.to_string())? = cfg;
        self.save()
    }

    pub fn should_auto_deny(&self, category: &str, severity: &str) -> bool {
        let cfg = self.auto_deny_config();
        if !cfg.enabled {
            return false;
        }
        if !cfg.categories.is_empty()
            && !cfg
                .categories
                .iter()
                .any(|c| c.eq_ignore_ascii_case(category))
        {
            return false;
        }
        severity_rank(severity) >= severity_rank(&cfg.min_severity)
    }

    pub fn geo_cidrs_snapshot(&self) -> Vec<GeoCidrRule> {
        load_geo_cidrs(&self.data_dir, &[])
    }

    pub fn reload_geo_cidrs(&self) -> Result<usize, String> {
        let cidrs = load_geo_cidrs(&self.data_dir, &[]);
        let n = cidrs.len();
        self.geo
            .write()
            .map_err(|e| e.to_string())?
            .reload_cidrs(cidrs);
        Ok(n)
    }

    pub fn reload_geo_mmdb(&self) -> Result<bool, String> {
        self.geo
            .write()
            .map_err(|e| e.to_string())?
            .reload_mmdb()
    }

    pub fn geo_status(&self) -> nerve_waf_geo::GeoStatus {
        self.geo.read().map(|g| g.status()).unwrap_or_else(|_| {
            nerve_waf_geo::GeoStatus {
                mmdb_loaded: false,
                mmdb_path: None,
                cidr_rules: 0,
                source: "auto".into(),
            }
        })
    }

    pub fn api_sec_config(&self) -> ApiSecConfig {
        self.api_sec.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_api_sec_config(&self, cfg: ApiSecConfig) -> Result<(), String> {
        *self.api_sec.write().map_err(|e| e.to_string())? = cfg;
        self.save()
    }

    pub fn tamper_config(&self) -> TamperConfig {
        self.tamper.config()
    }

    pub fn set_tamper_config(&self, cfg: TamperConfig) -> Result<(), String> {
        self.tamper.set_config(cfg);
        self.save()
    }

    pub fn tamper_guard(&self) -> &TamperGuard {
        &self.tamper
    }

    pub fn challenge_store(&self) -> &ChallengeStore {
        &self.challenge
    }

    pub fn rbac_config(&self) -> RbacConfig {
        self.rbac.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn set_rbac_config(&self, cfg: RbacConfig) -> Result<(), String> {
        *self.rbac.write().map_err(|e| e.to_string())? = cfg;
        self.save()
    }

    pub fn check_rbac(&self, key: Option<&str>, resource: &str, action: &str) -> bool {
        let cfg = self.rbac_config();
        let role = key
            .and_then(|k| nerve_waf_rbac::resolve_key(&cfg, k))
            .map(|c| c.role)
            .unwrap_or_else(|| "admin".into());
        nerve_waf_rbac::check_permission(&cfg, &role, resource, action)
    }

    pub fn usage_snapshot(&self) -> nerve_waf_license::UsageSnapshot {
        self.usage.snapshot()
    }

    pub fn usage_meter(&self) -> &UsageMeter {
        &self.usage
    }

    pub fn rules_pack_meta(&self) -> Vec<RulesPackMeta> {
        nerve_waf_rules_ops::list_pack_meta(&self.rules_packs.read().unwrap())
    }

    pub fn l7_rule_count(&self) -> usize {
        *self.l7_rule_count.read().unwrap()
    }

    pub fn reload_l7_rules(&self) -> Result<usize, String> {
        let crs = load_packs_from_dir(&self.rules_dir).map_err(|e| e.to_string())?;
        let patches = load_packs_from_dir(&self.patches_dir).map_err(|e| e.to_string())?;
        let ai = load_packs_from_dir(&self.ai_rules_dir).map_err(|e| e.to_string())?;
        let merged = self.merged_l7_rules_internal(&crs, &patches, &ai);
        let count = merged.len();
        *self.rules_packs.write().map_err(|e| e.to_string())? = crs;
        *self.patch_packs.write().map_err(|e| e.to_string())? = patches;
        *self.ai_packs.write().map_err(|e| e.to_string())? = ai;
        *self.l7_rule_count.write().map_err(|e| e.to_string())? = count;
        Ok(count)
    }

    pub fn merged_l7_rules(&self) -> Vec<CompiledRule> {
        let crs = self.rules_packs.read().unwrap().clone();
        let patches = self.patch_packs.read().unwrap().clone();
        let ai = self.ai_packs.read().unwrap().clone();
        self.merged_l7_rules_internal(&crs, &patches, &ai)
    }

    fn merged_l7_rules_internal(
        &self,
        crs: &[LoadedPack],
        patches: &[LoadedPack],
        ai: &[LoadedPack],
    ) -> Vec<CompiledRule> {
        let mut merged = merge_rules_with_patches(embedded_rules(), crs, patches);
        for pack in ai {
            if pack.meta.enabled {
                merged.extend(pack.rules.clone());
            }
        }
        dedupe_rules(merged)
    }

    pub fn rules_dir_path(&self) -> PathBuf {
        self.rules_dir.clone()
    }

    /// 应用绑定的 CRS 版本规则；未绑定时使用系统当前活跃规则集。
    pub fn merged_l7_rules_for_binding(
        &self,
        binding: Option<&nerve_waf_store::AppCrsBinding>,
    ) -> Vec<CompiledRule> {
        if let Some(b) = binding {
            let version_dir = crs_versions_dir(&self.data_dir).join(&b.crs_version_id);
            if version_dir.is_dir() {
                let crs = load_packs_from_dir(&version_dir).unwrap_or_default();
                let patches = self.patch_packs.read().unwrap().clone();
                let ai = self.ai_packs.read().unwrap().clone();
                return self.merged_l7_rules_internal(&crs, &patches, &ai);
            }
        }
        self.merged_l7_rules()
    }

    /// Edge dataplane 快照：可序列化 RawRule（与 merged_l7_rules_for_binding 同源）。
    pub fn raw_l7_rules_for_binding(
        &self,
        binding: Option<&nerve_waf_store::AppCrsBinding>,
    ) -> Vec<nerve_waf_l7::RawRule> {
        use nerve_waf_rules_ops::load_raw_rules_from_dir;
        use std::collections::HashMap;

        let mut by_id: HashMap<String, nerve_waf_l7::RawRule> = HashMap::new();
        if let Some(b) = binding {
            let version_dir = crs_versions_dir(&self.data_dir).join(&b.crs_version_id);
            if version_dir.is_dir() {
                if let Ok(rules) = load_raw_rules_from_dir(&version_dir) {
                    for r in rules {
                        by_id.insert(r.id.clone(), r);
                    }
                }
            }
        } else if let Ok(rules) = load_raw_rules_from_dir(&self.rules_dir) {
            for r in rules {
                by_id.insert(r.id.clone(), r);
            }
        }
        if let Ok(patch_rules) = load_raw_rules_from_dir(&self.patches_dir) {
            for r in patch_rules {
                by_id.insert(r.id.clone(), r);
            }
        }
        if let Ok(ai_rules) = load_raw_rules_from_dir(&self.ai_rules_dir) {
            for r in ai_rules {
                by_id.insert(r.id.clone(), r);
            }
        }
        by_id.into_values().collect()
    }

    pub fn system_crs_latest_ref(&self) -> Option<nerve_waf_store::SystemCrsVersionRef> {
        let active = self.merged_l7_rules();
        let versions = list_crs_versions(&crs_versions_dir(&self.data_dir)).ok()?;
        let latest = versions.first()?;
        Some(nerve_waf_store::SystemCrsVersionRef {
            crs_version_id: latest.id.clone(),
            crs_version_label: latest.crs_version.clone(),
            rule_count: active.len() as u32,
        })
    }

    pub fn crs_version_meta(&self, version_id: &str) -> Option<nerve_waf_rules_ops::CrsVersionRecord> {
        list_crs_versions(&crs_versions_dir(&self.data_dir))
            .ok()?
            .into_iter()
            .find(|v| v.id == version_id)
    }

    pub fn crs_version_rules_dir(&self, version_id: &str) -> PathBuf {
        crs_versions_dir(&self.data_dir).join(version_id)
    }

    pub fn merged_l7_rules_from_version_dir(&self, version_dir: &Path) -> Vec<CompiledRule> {
        let crs = load_packs_from_dir(version_dir).unwrap_or_default();
        let patches = self.patch_packs.read().unwrap().clone();
        let ai = self.ai_packs.read().unwrap().clone();
        self.merged_l7_rules_internal(&crs, &patches, &ai)
    }

    pub fn rules_inventory(&self, rule_list_limit: usize) -> nerve_waf_rules_ops::RulesInventory {
        let crs = self.rules_packs.read().unwrap();
        let patches = self.patch_packs.read().unwrap();
        let ai = self.ai_packs.read().unwrap();
        let merged = self.merged_l7_rules_internal(&crs, &patches, &ai);
        let embedded = embedded_rules().len();
        let (rules, rules_total) =
            nerve_waf_rules_ops::summarize_rules(&merged, rule_list_limit);
        nerve_waf_rules_ops::RulesInventory {
            rules_dir: self.rules_dir.to_string_lossy().into(),
            embedded_rules: embedded,
            total_rules: merged.len(),
            packs: nerve_waf_rules_ops::summarize_packs(&crs),
            patches: nerve_waf_rules_ops::summarize_packs(&patches),
            ai_packs: nerve_waf_rules_ops::summarize_packs(&ai),
            rules,
            rules_total,
            rules_truncated: merged.len() > rule_list_limit,
        }
    }

    pub fn patch_pack_meta(&self) -> Vec<RulesPackMeta> {
        nerve_waf_rules_ops::list_pack_meta(&self.patch_packs.read().unwrap())
    }

    pub fn crs_version_list(&self) -> Result<Vec<CrsVersionRecord>, String> {
        list_crs_versions(&crs_versions_dir(&self.data_dir)).map_err(|e| e.to_string())
    }

    pub fn crs_snapshot(&self, note: &str) -> Result<CrsVersionRecord, String> {
        snapshot_crs(
            &self.rules_dir,
            &crs_versions_dir(&self.data_dir),
            note,
            None,
        )
        .map_err(|e| e.to_string())
    }

    pub fn crs_rollback(&self, version_id: &str) -> Result<(CrsVersionRecord, usize), String> {
        rollback_crs(
            &self.rules_dir,
            &crs_versions_dir(&self.data_dir),
            version_id,
        )
        .map_err(|e| e.to_string())?;
        let n = self.reload_l7_rules()?;
        Ok((
            list_crs_versions(&crs_versions_dir(&self.data_dir))
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|v| v.id == version_id)
                .ok_or_else(|| "version metadata missing".to_string())?,
            n,
        ))
    }

    pub fn crs_update_latest(
        &self,
        crs_version: &str,
        paranoia_max: u8,
    ) -> Result<(CrsVersionRecord, nerve_waf_crs_import::ImportStats, usize), String> {
        let backup = snapshot_crs(
            &self.rules_dir,
            &crs_versions_dir(&self.data_dir),
            &format!("更新核心规则集 {crs_version} 前自动备份"),
            Some(crs_version),
        )
        .map_err(|e| e.to_string())?;

        let vendor = ensure_crs_vendor(&self.nerve_waf_root(), crs_version)?;
        let temp_dir = self.data_dir.join(".crs-import-tmp");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let stats = nerve_waf_crs_import::import_crs(&nerve_waf_crs_import::ImportOptions {
            crs_dir: vendor,
            out_dir: temp_dir.clone(),
            version: crs_version.trim_start_matches('v').into(),
            paranoia_max,
            dry_run: false,
            clean: true,
        })
        .map_err(|e| e.to_string())?;

        if stats.packs_written == 0 {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!(
                "CRS 导入 0 个规则包（扫描 {} 个文件，见 {} 条 SecRule，可解析 {} 条，编译失败 {} 条，写入 {} 条）。请确认 vendor/coreruleset 已下载且 paranoia<={paranoia_max} 未过滤全部规则",
                stats.files_scanned,
                stats.rules_seen,
                stats.rules_imported + stats.compile_failed,
                stats.compile_failed,
                stats.rules_imported
            ));
        }

        std::fs::create_dir_all(&self.rules_dir).map_err(|e| e.to_string())?;
        replace_rules_dir(&temp_dir, &self.rules_dir).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&temp_dir);

        let n = self.reload_l7_rules()?;
        Ok((backup, stats, n))
    }

    fn nerve_waf_root(&self) -> PathBuf {
        nerve_waf_root(&self.data_dir)
    }

    pub fn ai_rule_packs(&self) -> Vec<RulesPackMeta> {
        nerve_waf_rules_ops::list_pack_meta(&self.ai_packs.read().unwrap())
    }

    pub fn register_cluster(
        &self,
        cluster_id: String,
        relay_url: Option<String>,
        agent_count: u64,
    ) -> Result<(), String> {
        let mut clusters = self.clusters.write().map_err(|e| e.to_string())?;
        if let Some(c) = clusters.iter_mut().find(|c| c.cluster_id == cluster_id) {
            c.relay_url = relay_url;
            c.agent_count = agent_count;
        } else {
            clusters.push(ClusterRecord {
                cluster_id,
                relay_url,
                agent_count,
            });
        }
        self.save()
    }

    pub fn list_clusters(&self) -> Vec<ClusterRecord> {
        self.clusters.read().map(|c| c.clone()).unwrap_or_default()
    }
}

fn nerve_waf_root(data_dir: &Path) -> PathBuf {
    data_dir
        .canonicalize()
        .ok()
        .and_then(|d| d.parent().map(|p| p.to_path_buf()))
        .or_else(|| data_dir.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| data_dir.to_path_buf())
}

/// 仓库根目录（`Nerve-waf` 的上一级，如 `waf/`）
fn workspace_root(data_dir: &Path) -> Option<PathBuf> {
    nerve_waf_root(data_dir).parent().map(|p| p.to_path_buf())
}

fn normalize_rules_dir(data_dir: &Path, configured: Option<&str>, sub: &str) -> PathBuf {
    if sub == "crs" {
        if let Ok(p) = std::env::var("NERVE_RULES_DIR") {
            return PathBuf::from(p);
        }
    }
    if let Some(cfg) = configured.filter(|s| !s.is_empty()) {
        let p = PathBuf::from(cfg);
        if p.is_absolute() {
            return p;
        }
        if let Some(waf) = workspace_root(data_dir) {
            return waf.join(&p);
        }
        if p.is_dir() {
            return p;
        }
    }
    resolve_rules_dir(data_dir, sub)
}

fn resolve_rules_dir(data_dir: &Path, sub: &str) -> PathBuf {
    if sub == "crs" {
        if let Ok(p) = std::env::var("NERVE_RULES_DIR") {
            return PathBuf::from(p);
        }
    }
    if sub == "ai" {
        if let Ok(p) = std::env::var("NERVE_AI_RULES_DIR") {
            return PathBuf::from(p);
        }
    }
    if sub == "patches" {
        if let Ok(p) = std::env::var("NERVE_PATCHES_DIR") {
            return PathBuf::from(p);
        }
    }
    if let Some(waf) = workspace_root(data_dir) {
        return waf.join("rules").join(sub);
    }
    PathBuf::from(format!("rules/{sub}"))
}

fn load_geo_cidrs(data_dir: &Path, persisted: &[GeoCidrRule]) -> Vec<GeoCidrRule> {
    if !persisted.is_empty() {
        return persisted.to_vec();
    }
    let path = resolve_rules_dir(data_dir, "geoip").join("country_cidrs.json");
    if path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Vec<GeoCidrRule>>(&raw) {
                return v;
            }
        }
    }
    vec![]
}

fn severity_rank(s: &str) -> u8 {
    match s.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// 确保本地有可用的 OWASP CRS 源码树（含 `rules/`）。
///
/// 生产容器通常不装 git：优先 HTTPS 拉取 release tarball 并解压。
/// 可选环境变量：
/// - `NERVE_CRS_VENDOR_DIR`：已挂载/预置的 CRS 目录（含 rules/），跳过下载
/// - `NERVE_CRS_TARBALL_URL`：自定义 tarball URL，可含 `{version}` / `{tag}` 占位符
fn ensure_crs_vendor(nerve_waf_root: &Path, version: &str) -> Result<PathBuf, String> {
    let version = version.trim();
    if version.is_empty() {
        return Err("CRS 版本号不能为空".into());
    }

    if let Ok(raw) = std::env::var("NERVE_CRS_VENDOR_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.join("rules").is_dir() {
                return Ok(p);
            }
            return Err(format!(
                "环境变量 NERVE_CRS_VENDOR_DIR={} 无效：目录下未找到 rules/。请挂载完整的 OWASP CRS 源码树。",
                p.display()
            ));
        }
    }

    let vendor = nerve_waf_root.join("vendor/coreruleset");
    match download_crs_tarball(&vendor, version) {
        Ok(()) => return Ok(vendor),
        Err(download_err) => {
            for candidate in bundled_crs_candidates(nerve_waf_root) {
                if candidate.join("rules").is_dir() {
                    tracing::warn!(
                        error = %download_err,
                        path = %candidate.display(),
                        "CRS tarball 下载失败，回退使用本地/预置 CRS"
                    );
                    return Ok(candidate);
                }
            }
            if try_git_ensure_crs(&vendor, version).is_ok() {
                return Ok(vendor);
            }
            Err(download_err)
        }
    }
}

fn bundled_crs_candidates(nerve_waf_root: &Path) -> Vec<PathBuf> {
    vec![
        nerve_waf_root.join("vendor/coreruleset"),
        PathBuf::from("/opt/nerve-waf/vendor/coreruleset"),
        PathBuf::from("/opt/nerve-waf/data/vendor/coreruleset"),
    ]
}

fn crs_tarball_urls(version: &str) -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(u) = std::env::var("NERVE_CRS_TARBALL_URL") {
        let u = u.trim();
        if !u.is_empty() {
            urls.push(
                u.replace("{version}", version)
                    .replace("{tag}", version),
            );
        }
    }
    // codeload 比 github.com/archive 更稳定（部分环境访问后者会 SSL 中断）
    urls.push(format!(
        "https://codeload.github.com/coreruleset/coreruleset/tar.gz/refs/tags/{version}"
    ));
    urls.push(format!(
        "https://codeload.github.com/coreruleset/coreruleset/tar.gz/{version}"
    ));
    urls.push(format!(
        "https://github.com/coreruleset/coreruleset/archive/refs/tags/{version}.tar.gz"
    ));
    urls
}

fn download_crs_tarball(vendor: &Path, version: &str) -> Result<(), String> {
    let parent = vendor
        .parent()
        .ok_or_else(|| "无效的 CRS vendor 路径".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("创建 vendor 目录失败: {e}"))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .connect_timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(concat!("nerve-waf-api/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let mut last_err = String::new();
    let mut payload: Option<Vec<u8>> = None;
    for url in crs_tarball_urls(version) {
        tracing::info!(%url, %version, "downloading OWASP CRS tarball");
        match client.get(&url).send() {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    last_err = format!("HTTP {status} from {url}");
                    tracing::warn!(%url, %status, "CRS tarball URL failed");
                    continue;
                }
                match response.bytes() {
                    Ok(b) if b.len() >= 1024 => {
                        payload = Some(b.to_vec());
                        break;
                    }
                    Ok(b) => {
                        last_err = format!("响应过小（{} 字节）from {url}", b.len());
                    }
                    Err(e) => {
                        last_err = format!("读取响应失败 from {url}: {e}");
                    }
                }
            }
            Err(e) => {
                last_err = format!("请求失败 from {url}: {e}");
                tracing::warn!(%url, error = %e, "CRS tarball request failed");
            }
        }
    }

    let payload = payload.ok_or_else(|| {
        format!(
            "无法下载 OWASP CRS {version}（{last_err}）。\
请确认运行环境可访问 GitHub/codeload，或挂载预置规则到 /opt/nerve-waf/vendor/coreruleset，\
也可设置 NERVE_CRS_VENDOR_DIR / NERVE_CRS_TARBALL_URL。"
        )
    })?;

    let staging = parent.join(format!(
        ".coreruleset-staging-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

    {
        let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(payload.as_slice()));
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(&staging).map_err(|e| {
            format!(
                "解压 OWASP CRS {version} 失败: {e}。请确认 tarball 完整，或改用 NERVE_CRS_VENDOR_DIR。"
            )
        })?;
    }

    let extracted = find_crs_root(&staging).ok_or_else(|| {
        format!(
            "解压 OWASP CRS {version} 后未找到 rules/ 目录。请检查 NERVE_CRS_TARBALL_URL 是否指向正确的 CRS 包。"
        )
    })?;

    let backup = parent.join(format!(
        ".coreruleset-backup-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&backup);
    if vendor.exists() {
        std::fs::rename(vendor, &backup).map_err(|e| {
            format!("备份旧 CRS 目录失败: {e}")
        })?;
    }

    if let Err(e) = std::fs::rename(&extracted, vendor) {
        // 跨设备时 rename 可能失败，回退为递归复制
        if let Err(copy_err) = copy_dir_recursive(&extracted, vendor) {
            if backup.exists() {
                let _ = std::fs::rename(&backup, vendor);
            }
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!(
                "安装 CRS 到 {} 失败: rename={e}; copy={copy_err}",
                vendor.display()
            ));
        }
    }

    let _ = std::fs::remove_dir_all(&backup);
    let _ = std::fs::remove_dir_all(&staging);
    tracing::info!(path = %vendor.display(), %version, "OWASP CRS vendor ready");
    Ok(())
}

fn find_crs_root(dir: &Path) -> Option<PathBuf> {
    if dir.join("rules").is_dir() {
        return Some(dir.to_path_buf());
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.join("rules").is_dir() {
                return Some(path);
            }
            if let Some(found) = find_crs_root(&path) {
                return Some(found);
            }
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 本地开发机可选回退：系统已安装 git 时 clone/fetch。
fn try_git_ensure_crs(vendor: &Path, version: &str) -> Result<(), String> {
    let git_ok = std::process::Command::new("git")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !git_ok {
        return Err("git unavailable".into());
    }

    if vendor.join("rules").is_dir() {
        if vendor.join(".git").is_dir() {
            let fetch = std::process::Command::new("git")
                .args(["-C"])
                .arg(vendor)
                .args(["fetch", "--tags", "--depth", "1"])
                .status()
                .map_err(|e| format!("git fetch 失败: {e}"))?;
            if fetch.success() {
                let _ = std::process::Command::new("git")
                    .args(["-C"])
                    .arg(vendor)
                    .args(["checkout", version])
                    .status();
            }
        }
        return Ok(());
    }

    std::fs::create_dir_all(vendor.parent().ok_or("invalid vendor path")?)
        .map_err(|e| e.to_string())?;
    let status = std::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            version,
            "https://github.com/coreruleset/coreruleset.git",
        ])
        .arg(vendor)
        .status()
        .map_err(|e| format!("git clone 失败: {e}"))?;
    if !status.success() {
        return Err(format!("git clone coreruleset {version} 失败"));
    }
    Ok(())
}

fn dedupe_rules(rules: Vec<CompiledRule>) -> Vec<CompiledRule> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    rules
        .into_iter()
        .filter(|r| seen.insert(r.id.clone()))
        .collect()
}
