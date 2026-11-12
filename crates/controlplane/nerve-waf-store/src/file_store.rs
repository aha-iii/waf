use crate::StoreError;
use chrono::Utc;
use nerve_waf_sdk::{EventQuery, EventQueryResult, StatsOverview, WafEventRecord};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

pub struct FileEventStore {
    path: PathBuf,
    cache: RwLock<Vec<WafEventRecord>>,
}

impl FileEventStore {
    pub fn new(path: PathBuf) -> Self {
        let cache = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            path,
            cache: RwLock::new(cache),
        }
    }

    fn persist(&self, events: &[WafEventRecord]) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(events)?;
        std::fs::write(&self.path, raw)?;
        Ok(())
    }
}

impl FileEventStore {
    pub async fn insert_batch(&self, events: &[WafEventRecord]) -> Result<usize, StoreError> {
        if events.is_empty() {
            return Ok(0);
        }
        let mut cache = self.cache.write().unwrap();
        cache.extend(events.iter().cloned());
        if cache.len() > 50_000 {
            let drop = cache.len() - 50_000;
            cache.drain(0..drop);
        }
        self.persist(&cache)?;
        Ok(events.len())
    }

    pub async fn query(&self, q: EventQuery) -> Result<EventQueryResult, StoreError> {
        let cache = self.cache.read().unwrap();
        let mut filtered: Vec<WafEventRecord> = cache
            .iter()
            .filter(|e| {
                q.category
                    .as_ref()
                    .map(|c| &e.category == c)
                    .unwrap_or(true)
                    && q.action.as_ref().map(|a| &e.action == a).unwrap_or(true)
                    && q.src_ip
                        .as_ref()
                        .map(|ip| e.src_ip.as_deref() == Some(ip.as_str()))
                        .unwrap_or(true)
                    && q.site_id
                        .as_ref()
                        .map(|sid| &e.site_id == sid)
                        .unwrap_or(true)
                    && q.layer
                        .as_ref()
                        .map(|layer| &e.layer == layer)
                        .unwrap_or(true)
                    && q.exclude_categories
                        .as_ref()
                        .map(|ex| !ex.iter().any(|c| c == &e.category))
                        .unwrap_or(true)
            })
            .cloned()
            .collect();
        filtered.sort_by(|a, b| b.ts.cmp(&a.ts));
        let total = filtered.len() as u64;
        let limit = q.limit.clamp(1, 500) as usize;
        let offset = q.offset as usize;
        let events = filtered.into_iter().skip(offset).take(limit).collect();
        Ok(EventQueryResult { total, events })
    }

    pub async fn stats_overview(&self) -> Result<StatsOverview, StoreError> {
        self.stats_since(Utc::now() - chrono::Duration::hours(24))
            .await
    }

    pub async fn stats_since(
        &self,
        since: chrono::DateTime<Utc>,
    ) -> Result<StatsOverview, StoreError> {
        let cache = self.cache.read().unwrap();
        let recent: Vec<_> = cache.iter().filter(|e| e.ts >= since).collect();
        let total_events = recent.len() as u64;
        let blocked = recent.iter().filter(|e| e.action == "block").count() as u64;
        let logged = recent.iter().filter(|e| e.action == "log").count() as u64;

        let mut cat_map: HashMap<String, u64> = HashMap::new();
        for e in &recent {
            *cat_map.entry(e.category.clone()).or_default() += 1;
        }
        let mut by_category: Vec<_> = cat_map
            .into_iter()
            .map(|(category, count)| nerve_waf_sdk::CategoryCount { category, count })
            .collect();
        by_category.sort_by(|a, b| b.count.cmp(&a.count));

        let mut ip_map: HashMap<String, u64> = HashMap::new();
        for e in &recent {
            if let Some(ip) = &e.src_ip {
                *ip_map.entry(ip.clone()).or_default() += 1;
            }
        }
        let mut top_ips: Vec<_> = ip_map
            .into_iter()
            .map(|(src_ip, count)| nerve_waf_sdk::IpCount { src_ip, count })
            .collect();
        top_ips.sort_by(|a, b| b.count.cmp(&a.count));
        top_ips.truncate(10);

        Ok(StatsOverview {
            total_events,
            blocked,
            logged,
            by_category,
            top_ips,
        })
    }

    pub async fn network_stats_since(
        &self,
        since: chrono::DateTime<Utc>,
        days: i64,
    ) -> Result<serde_json::Value, StoreError> {
        let cache = self.cache.read().unwrap();
        Ok(crate::network_stats::file_network_stats(
            &cache, since, days,
        ))
    }
}
