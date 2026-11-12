use nerve_waf_auth::AuthState;
use nerve_waf_client::NerveClient;
use nerve_waf_plugin_host::{resolve_plugins_dir, HostedL7, PluginHost};
use nerve_waf_sdk::WafFileConfig;
use nerve_waf_store::CustomRulesStore;
use nerve_waf_store::ProtectedSitesStore;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::dataplane::BypassInspectRuntime;
use crate::platform;
use crate::triage_ai;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct BootstrapOptions {
    pub config_path: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub database_url: Option<String>,
    pub nerve_url: Option<String>,
}

impl BootstrapOptions {
    pub fn from_env_and_config(config_path: Option<PathBuf>) -> Self {
        let file = WafFileConfig::load_or_default(config_path.as_deref());
        let node = &file.waf;
        Self {
            config_path,
            data_dir: std::env::var("NERVE_WAF_DATA")
                .ok()
                .map(PathBuf::from)
                .or_else(|| Some(PathBuf::from(&node.data_dir))),
            database_url: pick_database_url(
                std::env::var("DATABASE_URL").ok(),
                node.database.url.clone(),
            ),
            nerve_url: std::env::var("NERVE_URL")
                .ok()
                .or_else(|| Some(node.nerve_url.clone())),
        }
    }
}

fn pick_database_url(env: Option<String>, from_config: Option<String>) -> Option<String> {
    if let Some(url) = env.filter(|u| !u.trim().is_empty()) {
        return Some(url);
    }
    from_config.filter(|u| !u.trim().is_empty())
}

pub async fn build_app_state(opts: BootstrapOptions) -> Arc<AppState> {
    let data_path = opts.data_dir.unwrap_or_else(|| PathBuf::from("./data"));
    let database_url = opts.database_url;
    let nerve_url = opts
        .nerve_url
        .unwrap_or_else(|| "http://127.0.0.1:8766".into());

    let config = nerve_waf_store::open_config_backend(database_url.as_deref(), &data_path)
        .await
        .expect("open config backend");

    let modules = config.load_modules().await.unwrap_or_default();
    let nerve = NerveClient::new(&nerve_url);

    let plugins_dir = resolve_plugins_dir(Some(config.data_dir().as_path()));
    let mut plugin_host = PluginHost::new(plugins_dir);
    let reload_report = plugin_host.reload();
    tracing::info!(
        plugins_dir = %reload_report.plugins_dir,
        json = reload_report.json_packs,
        lua = reload_report.lua_plugins,
        rules = reload_report.total_rules,
        "plugin host loaded (agent inspect)"
    );

    let events =
        nerve_waf_store::open_event_store(database_url.as_deref(), config.data_dir().as_path())
            .await
            .expect("open event store");

    // Topology / Docker inventory moved to Nerve-control (:8780). WAF no longer holds topology Store.
    // Emergency rollback: WAF_ENABLE_LEGACY_TOPOLOGY=1 still attaches PG topology (not recommended).
    let topology = if matches!(
        std::env::var("WAF_ENABLE_LEGACY_TOPOLOGY").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    ) {
        tracing::warn!("WAF_ENABLE_LEGACY_TOPOLOGY=1 — topology still on waf-api (deprecated)");
        nerve_waf_store::topology_from_events(events.as_ref())
    } else {
        None
    };
    let (platform, protected_sites_store) = {
        let mut p = platform::PlatformServices::load(config.data_dir().as_path());
        let store = if let Some(ref url) = database_url {
            match ProtectedSitesStore::open(url).await {
                Ok(store) => {
                    if let Err(e) = hydrate_protected_sites_with_store(&mut p, &store).await {
                        tracing::warn!("protected sites PostgreSQL hydrate: {e}");
                        None
                    } else {
                        p.set_sites_persist_db(true);
                        Some(Arc::new(store))
                    }
                }
                Err(e) => {
                    tracing::warn!("open protected sites store: {e}");
                    None
                }
            }
        } else {
            None
        };
        (Arc::new(p), store)
    };
    let users_store = if let Some(ref url) = database_url {
        match nerve_waf_store::UsersStore::open(url).await {
            Ok(store) => {
                let username = std::env::var("NERVE_AUTH_BOOTSTRAP_USER").unwrap_or_else(|_| {
                    std::env::var("NERVE_AUTH_USER").unwrap_or_else(|_| "admin".into())
                });
                let password =
                    std::env::var("NERVE_AUTH_BOOTSTRAP_PASSWORD").unwrap_or_else(|_| {
                        std::env::var("NERVE_AUTH_PASSWORD").unwrap_or_else(|_| "admin".into())
                    });
                if let Err(e) = store.ensure_bootstrap_admin(&username, &password).await {
                    tracing::warn!("bootstrap admin user: {e}");
                }
                Some(Arc::new(store))
            }
            Err(e) => {
                tracing::warn!("open users store: {e}");
                None
            }
        }
    } else {
        None
    };
    let demo_leads_store = if let Some(ref url) = database_url {
        match nerve_waf_store::DemoLeadsStore::open(url).await {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                tracing::warn!("open demo leads store: {e}");
                None
            }
        }
    } else {
        None
    };
    let hosted_l7 = HostedL7::from_host(&plugin_host, &modules, platform.merged_l7_rules());

    let custom_rules_store = Arc::new(
        CustomRulesStore::open(database_url.as_deref(), config.data_dir().as_path())
            .await
            .expect("open custom rules store"),
    );
    let initial_custom_rules = custom_rules_store
        .load_active_rules_for("default")
        .await
        .unwrap_or_default();

    let ai_settings = Arc::new(
        nerve_waf_store::AiSettingsStore::open(
            database_url.as_deref(),
            config.data_dir().as_path(),
        )
        .await
        .expect("open ai settings store"),
    );
    let triage_ai = triage_ai::TriageAiService::new(ai_settings.clone());
    let semantic_stats = Arc::new(
        nerve_waf_store::SemanticStatsStore::open(
            database_url.as_deref(),
            config.data_dir().as_path(),
        )
        .await
        .expect("open semantic stats store"),
    );
    semantic_stats.clone().spawn_flush_task();

    let traffic =
        nerve_waf_store::open_traffic_store(database_url.as_deref(), config.data_dir().as_path())
            .await
            .expect("open traffic store");

    let bypass_inspect = Arc::new(BypassInspectRuntime::new(config.data_dir().as_path()));

    let state = Arc::new(AppState {
        config: config.clone(),
        nerve,
        nerve_url,
        events,
        plugin_host: Arc::new(RwLock::new(plugin_host)),
        hosted_l7: Arc::new(RwLock::new(hosted_l7)),
        database_url,
        topology,
        platform,
        protected_sites_store,
        users_store,
        demo_leads_store,
        auth: Arc::new(AuthState::from_env()),
        alert_client: crate::controlplane::alert_integration::new_alert_client(),
        custom_rules_store,
        custom_rules_cache: Arc::new(RwLock::new(initial_custom_rules)),
        ai_settings,
        triage_ai,
        semantic_stats,
        traffic,
        dataplane_gen: crate::dataplane::DataplaneGeneration::new(),
        last_edge_ack_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        bypass_inspect,
    });

    if let Err(e) = state.config.ensure_app_provisioned("default").await {
        tracing::warn!("provision default app: {e}");
    }
    if let Err(e) = state.ai_settings.refresh_site_from_store("default").await {
        tracing::warn!("refresh default pipeline cache: {e}");
    }

    crate::dataplane::bump_after_mutation(state.as_ref());

    state
}

async fn hydrate_protected_sites_with_store(
    platform: &mut platform::PlatformServices,
    store: &ProtectedSitesStore,
) -> Result<(), String> {
    let db_values = store.list().await.map_err(|e| e.to_string())?;
    if !db_values.is_empty() {
        let sites: Vec<platform::ProtectedSite> = db_values
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        tracing::info!(
            count = sites.len(),
            "protected sites loaded from PostgreSQL"
        );
        platform.set_sites(sites);
        return Ok(());
    }
    let file_sites = platform.file_sites_snapshot();
    if file_sites.is_empty() {
        tracing::info!("protected sites: PostgreSQL and platform.json both empty");
        return Ok(());
    }
    let values: Vec<serde_json::Value> = file_sites
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    store
        .replace_all(&values, "bootstrap-migrate")
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(
        count = file_sites.len(),
        "migrated protected sites from platform.json to PostgreSQL"
    );
    Ok(())
}

pub fn load_env_files() {
    if dotenvy::dotenv().is_ok() {
        return;
    }
    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..8 {
            let candidate = dir.join(".env");
            if candidate.is_file() {
                let _ = dotenvy::from_path(&candidate);
                return;
            }
            if !dir.pop() {
                break;
            }
        }
    }
}
