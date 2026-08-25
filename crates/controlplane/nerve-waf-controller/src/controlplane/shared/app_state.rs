use nerve_waf_auth::AuthState;
use crate::controlplane::alert_integration::AlertClient;
use nerve_waf_client::NerveClient;
use nerve_waf_l7::CustomRule;
use nerve_waf_plugin_host::{HostedL7, PluginHost};
use nerve_waf_store::{ConfigBackend, CustomRulesStore, DemoLeadsStore, ProtectedSitesStore, UsersStore};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};

use crate::dataplane::generation::DataplaneGeneration;
use crate::platform;
use crate::dataplane::BypassInspectRuntime;
use crate::triage_ai;

#[derive(Clone)]
pub struct AppState {
    pub config: ConfigBackend,
    pub nerve: NerveClient,
    pub nerve_url: String,
    pub events: Arc<nerve_waf_store::EventStore>,
    pub plugin_host: Arc<RwLock<PluginHost>>,
    pub hosted_l7: Arc<RwLock<HostedL7>>,
    pub database_url: Option<String>,
    pub topology: Option<Arc<nerve_waf_store::TopologyStore>>,
    pub platform: Arc<platform::PlatformServices>,
    pub protected_sites_store: Option<Arc<ProtectedSitesStore>>,
    pub users_store: Option<Arc<UsersStore>>,
    pub demo_leads_store: Option<Arc<DemoLeadsStore>>,
    pub auth: Arc<AuthState>,
    pub alert_client: AlertClient,
    pub custom_rules_store: Arc<CustomRulesStore>,
    pub custom_rules_cache: Arc<RwLock<Vec<CustomRule>>>,
    pub ai_settings: Arc<nerve_waf_store::AiSettingsStore>,
    pub triage_ai: Arc<triage_ai::TriageAiService>,
    pub semantic_stats: Arc<nerve_waf_store::SemanticStatsStore>,
    pub traffic: Arc<nerve_waf_store::TrafficStore>,
    pub dataplane_gen: DataplaneGeneration,
    pub last_edge_ack_generation: Arc<AtomicU64>,
    pub bypass_inspect: Arc<BypassInspectRuntime>,
}

impl AppState {
    pub async fn reload_custom_rules_cache(&self) {
        match self.custom_rules_store.load_active_rules_for("default").await {
            Ok(rules) => {
                *self.custom_rules_cache.write().unwrap() = rules;
            }
            Err(e) => tracing::warn!("reload custom rules failed: {e}"),
        }
    }

    pub async fn refresh_l7_from_store(&self) {
        let modules = self
            .config
            .load_modules()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("load modules failed: {e}");
                nerve_waf_sdk::ProtectionModules::default()
            });
        let crs = self.platform.merged_l7_rules();
        let host = self.plugin_host.read().unwrap();
        let hosted = HostedL7::from_host(&host, &modules, crs);
        *self.hosted_l7.write().unwrap() = hosted;
    }

    pub fn reload_plugins_and_l7(&self) -> nerve_waf_sdk::PluginReloadReport {
        let mut host = self.plugin_host.write().unwrap();
        host.reload()
    }

    pub async fn reload_plugins_and_l7_async(&self) -> nerve_waf_sdk::PluginReloadReport {
        let modules = self
            .config
            .load_modules()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("load modules failed: {e}");
                nerve_waf_sdk::ProtectionModules::default()
            });
        let crs = self.platform.merged_l7_rules();
        let mut host = self.plugin_host.write().unwrap();
        let report = host.reload();
        let hosted = HostedL7::from_host(&host, &modules, crs);
        *self.hosted_l7.write().unwrap() = hosted;
        self.bypass_inspect.apply_modules(modules);
        self.bypass_inspect.refresh_global_overlay(self);
        report
    }

    pub fn l7_rule_count(&self) -> usize {
        self.hosted_l7.read().unwrap().rule_count()
    }
}
