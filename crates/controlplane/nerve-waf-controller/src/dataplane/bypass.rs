//! HTTP bypass / agent-local inspect hot path: align with Edge snapshot + PreparedSiteInspect cache.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use nerve_waf_core::InspectVerdict;
use nerve_waf_dataplane::{GlobalInspectOverlay, SiteInspectSnapshot};
use nerve_waf_embed::{
    EmbedInspectRequest, EmbeddedWaf, InspectPreparedOptions, SharedInspectCache,
};
use std::path::Path;

use nerve_waf_plugin_host::resolve_plugins_dir;

use crate::site_inspect::{load_global_inspect_overlay, load_site_inspect_inputs};
use crate::AppState;

#[derive(Default)]
struct SiteSnapshotCache {
    generation: u64,
    sites: HashMap<String, SiteInspectSnapshot>,
    global: GlobalInspectOverlay,
}

pub struct BypassInspectRuntime {
    site_cache: RwLock<SiteSnapshotCache>,
    prepared_cache: SharedInspectCache,
    embedded: Arc<EmbeddedWaf>,
}

impl BypassInspectRuntime {
    pub fn new(data_dir: &Path) -> Self {
        let plugins_dir = resolve_plugins_dir(Some(data_dir));
        let embedded = Arc::new(EmbeddedWaf::new(Some(plugins_dir)));
        Self {
            site_cache: RwLock::new(SiteSnapshotCache::default()),
            prepared_cache: nerve_waf_embed::new_shared_inspect_cache(),
            embedded,
        }
    }

    pub fn refresh_global_overlay(&self, state: &AppState) {
        self.embedded
            .apply_global_overlay(load_global_inspect_overlay(state));
    }

    pub fn apply_modules(&self, modules: nerve_waf_sdk::ProtectionModules) {
        self.embedded.apply_modules(modules);
    }

    async fn site_snapshot(
        &self,
        state: &AppState,
        site_id: &str,
        generation: u64,
    ) -> Result<SiteInspectSnapshot, String> {
        {
            let guard = self.site_cache.read().unwrap();
            if guard.generation == generation {
                if let Some(s) = guard.sites.get(site_id) {
                    return Ok(s.clone());
                }
            }
        }
        let inputs = load_site_inspect_inputs(state, site_id).await?;
        let global = load_global_inspect_overlay(state);
        let mut guard = self.site_cache.write().unwrap();
        if guard.generation != generation {
            guard.generation = generation;
            guard.sites.clear();
            guard.global = global.clone();
            self.embedded.apply_global_overlay(global);
        }
        guard
            .sites
            .insert(site_id.to_string(), inputs.snapshot.clone());
        Ok(inputs.snapshot)
    }

    fn cached_global(&self, generation: u64) -> GlobalInspectOverlay {
        let guard = self.site_cache.read().unwrap();
        if guard.generation == generation {
            return guard.global.clone();
        }
        GlobalInspectOverlay::default()
    }

    pub async fn inspect_verdict(
        &self,
        state: &AppState,
        site_id: &str,
        generation: u64,
        embed_req: EmbedInspectRequest,
        bot_challenge_verified: bool,
        trace_id: Option<String>,
    ) -> Result<InspectVerdict, String> {
        let site_snap = self.site_snapshot(state, site_id, generation).await?;
        let global = self.cached_global(generation);
        let waf = Arc::clone(&self.embedded);
        let cache = Arc::clone(&self.prepared_cache);
        let site_id_owned = site_id.to_string();

        tokio::task::spawn_blocking(move || {
            let prepared = {
                let mut guard = cache.lock().map_err(|e| e.to_string())?;
                if let Some(p) = guard.get_prepared(generation, &site_id_owned) {
                    p
                } else {
                    drop(guard);
                    let prepared = waf.prepare_site_inspect(&site_snap);
                    let mut guard = cache.lock().map_err(|e| e.to_string())?;
                    if let Some(p) = guard.get_prepared(generation, &site_id_owned) {
                        p
                    } else {
                        guard.put_prepared(generation, &site_id_owned, prepared)
                    }
                }
            };
            Ok(waf.inspect_prepared_verdict(
                embed_req,
                &prepared,
                &global,
                InspectPreparedOptions {
                    bot_challenge_verified,
                    trace_id,
                },
            ))
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
