use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::PreparedSiteInspect;

#[derive(Default)]
pub struct SiteInspectCache {
    generation: u64,
    sites: HashMap<String, Arc<PreparedSiteInspect>>,
}

impl SiteInspectCache {
    pub fn get_prepared(&self, generation: u64, site_id: &str) -> Option<Arc<PreparedSiteInspect>> {
        if self.generation != generation {
            return None;
        }
        self.sites.get(site_id).cloned()
    }

    pub fn put_prepared(
        &mut self,
        generation: u64,
        site_id: &str,
        prepared: PreparedSiteInspect,
    ) -> Arc<PreparedSiteInspect> {
        if self.generation != generation {
            self.generation = generation;
            self.sites.clear();
        }
        let arc = Arc::new(prepared);
        self.sites.insert(site_id.to_string(), Arc::clone(&arc));
        arc
    }

    pub fn get_or_prepare(
        &mut self,
        generation: u64,
        site_id: &str,
        prepare: impl FnOnce() -> PreparedSiteInspect,
    ) -> Arc<PreparedSiteInspect> {
        if self.generation != generation {
            self.generation = generation;
            self.sites.clear();
        }
        self.sites
            .entry(site_id.to_string())
            .or_insert_with(|| Arc::new(prepare()))
            .clone()
    }
}

pub type SharedInspectCache = Arc<Mutex<SiteInspectCache>>;

pub fn new_shared_inspect_cache() -> SharedInspectCache {
    Arc::new(Mutex::new(SiteInspectCache::default()))
}
