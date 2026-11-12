use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSample {
    pub ts: String,
    pub param_path: String,
    pub decode_chain: Vec<String>,
    pub semantic_score: u8,
    pub engines: Vec<String>,
    pub uri: String,
    pub hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticCounters {
    pub inspect_total: u64,
    pub hits: u64,
    pub misses: u64,
    pub skipped_whitelist: u64,
    pub skipped_timeout: u64,
    pub slow_path_runs: u64,
    pub slow_path_skipped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFeedback {
    pub ts: String,
    #[serde(default = "default_site")]
    pub site_id: String,
    pub event_id: Option<String>,
    pub verdict: String,
    pub note: Option<String>,
    #[serde(default)]
    pub param_path: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub applied_action: Option<String>,
}

fn default_site() -> String {
    "default".into()
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticStatsSnapshot {
    pub site_id: String,
    pub counters: SemanticCounters,
    pub hit_rate_pct: f64,
    pub recent_samples: Vec<SemanticSample>,
    pub feedback: Vec<SemanticFeedback>,
    pub backend: &'static str,
}

/// Stats interface used on detect hot path (in-memory + persistent impls).
pub trait SemanticStatsRecorder: Send + Sync {
    fn record_inspect(&self, site_id: &str);
    fn record_hit(&self, site_id: &str, sample: SemanticSample);
    fn record_miss(&self, site_id: &str, sample: Option<SemanticSample>);
    fn record_whitelist_skip(&self, site_id: &str);
    fn record_timeout(&self, site_id: &str);
    fn record_slow_path(&self, site_id: &str, ran: bool);
    fn add_feedback(&self, fb: SemanticFeedback);
    fn feedback_for_site(&self, site_id: &str) -> Vec<SemanticFeedback>;
    fn snapshot(&self, site_id: &str) -> SemanticStatsSnapshot;
    fn backend_name(&self) -> &'static str;
}

#[derive(Default)]
struct SiteBucket {
    counters: SemanticCounters,
    samples: VecDeque<SemanticSample>,
    feedback: VecDeque<SemanticFeedback>,
}

/// Pure in-memory impl (tests / fallback).
pub struct InMemorySemanticStats {
    sites: RwLock<std::collections::HashMap<String, SiteBucket>>,
}

impl InMemorySemanticStats {
    pub fn new() -> Self {
        Self {
            sites: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn load_site(
        &self,
        site_id: &str,
        counters: SemanticCounters,
        samples: Vec<SemanticSample>,
        feedback: Vec<SemanticFeedback>,
    ) {
        let mut sites = self.sites.write().unwrap_or_else(|e| e.into_inner());
        let bucket = sites.entry(site_id.to_string()).or_default();
        bucket.counters = counters;
        bucket.samples = samples.into_iter().collect();
        bucket.feedback = feedback.into_iter().collect();
    }

    fn with_bucket<F, R>(&self, site_id: &str, f: F) -> R
    where
        F: FnOnce(&mut SiteBucket) -> R,
    {
        let mut sites = self.sites.write().unwrap_or_else(|e| e.into_inner());
        f(sites.entry(site_id.to_string()).or_default())
    }
}

impl Default for InMemorySemanticStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticStatsRecorder for InMemorySemanticStats {
    fn record_inspect(&self, site_id: &str) {
        self.with_bucket(site_id, |b| b.counters.inspect_total += 1);
    }

    fn record_hit(&self, site_id: &str, sample: SemanticSample) {
        self.with_bucket(site_id, |b| {
            b.counters.hits += 1;
            b.samples.push_front(sample);
            while b.samples.len() > 50 {
                b.samples.pop_back();
            }
        });
    }

    fn record_miss(&self, site_id: &str, sample: Option<SemanticSample>) {
        self.with_bucket(site_id, |b| {
            b.counters.misses += 1;
            if let Some(s) = sample {
                b.samples.push_front(s);
                while b.samples.len() > 50 {
                    b.samples.pop_back();
                }
            }
        });
    }

    fn record_whitelist_skip(&self, site_id: &str) {
        self.with_bucket(site_id, |b| b.counters.skipped_whitelist += 1);
    }

    fn record_timeout(&self, site_id: &str) {
        self.with_bucket(site_id, |b| b.counters.skipped_timeout += 1);
    }

    fn record_slow_path(&self, site_id: &str, ran: bool) {
        self.with_bucket(site_id, |b| {
            if ran {
                b.counters.slow_path_runs += 1;
            } else {
                b.counters.slow_path_skipped += 1;
            }
        });
    }

    fn add_feedback(&self, fb: SemanticFeedback) {
        let site_id = fb.site_id.clone();
        self.with_bucket(&site_id, |b| {
            b.feedback.push_front(fb);
            while b.feedback.len() > 100 {
                b.feedback.pop_back();
            }
        });
    }

    fn feedback_for_site(&self, site_id: &str) -> Vec<SemanticFeedback> {
        self.sites
            .read()
            .ok()
            .and_then(|s| s.get(site_id).map(|b| b.feedback.iter().cloned().collect()))
            .unwrap_or_default()
    }

    fn snapshot(&self, site_id: &str) -> SemanticStatsSnapshot {
        let sites = self.sites.read().ok();
        let bucket = sites.as_ref().and_then(|s| s.get(site_id));
        let counters = bucket.map(|b| b.counters.clone()).unwrap_or_default();
        let samples: Vec<_> = bucket
            .map(|b| b.samples.iter().cloned().collect())
            .unwrap_or_default();
        let feedback: Vec<_> = bucket
            .map(|b| b.feedback.iter().cloned().collect())
            .unwrap_or_default();
        let hit_rate = if counters.inspect_total > 0 {
            (counters.hits as f64 / counters.inspect_total as f64) * 100.0
        } else {
            0.0
        };
        SemanticStatsSnapshot {
            site_id: site_id.to_string(),
            counters,
            hit_rate_pct: (hit_rate * 10.0).round() / 10.0,
            recent_samples: samples,
            feedback,
            backend: "memory",
        }
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

pub struct SemanticStatsHandle<'a> {
    pub site_id: &'a str,
    pub recorder: &'a dyn SemanticStatsRecorder,
}

impl<'a> SemanticStatsHandle<'a> {
    pub fn record_inspect(&self) {
        self.recorder.record_inspect(self.site_id);
    }

    pub fn record_hit(&self, sample: SemanticSample) {
        self.recorder.record_hit(self.site_id, sample);
    }

    pub fn record_miss(&self, sample: Option<SemanticSample>) {
        self.recorder.record_miss(self.site_id, sample);
    }

    pub fn record_whitelist_skip(&self) {
        self.recorder.record_whitelist_skip(self.site_id);
    }

    pub fn record_timeout(&self) {
        self.recorder.record_timeout(self.site_id);
    }

    pub fn record_slow_path(&self, ran: bool) {
        self.recorder.record_slow_path(self.site_id, ran);
    }
}

pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

pub struct InspectTimer {
    started: Instant,
    budget_ms: u32,
}

impl InspectTimer {
    pub fn new(budget_ms: u32) -> Self {
        Self {
            started: Instant::now(),
            budget_ms,
        }
    }

    pub fn expired(&self) -> bool {
        self.started.elapsed().as_millis() as u32 >= self.budget_ms
    }
}
