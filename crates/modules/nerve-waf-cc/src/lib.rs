//! CC / rate limiting with an in-memory token bucket (IP / Host / URI dimensions).

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_limit")]
    pub requests_per_minute: u32,
    #[serde(default = "default_burst")]
    pub burst: u32,
    #[serde(default)]
    pub block_duration_sec: u64,
    /// Optional Redis URL; when set, CC counters use Redis (distributed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis_url: Option<String>,
}

impl Default for CcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: default_limit(),
            burst: default_burst(),
            block_duration_sec: 300,
            redis_url: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}
fn default_limit() -> u32 {
    120
}
fn default_burst() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcCheckRequest {
    pub src_ip: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CcCheckResult {
    pub allowed: bool,
    pub remaining: u32,
    pub retry_after_sec: Option<u64>,
    pub reason: Option<String>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

pub struct CcEngine {
    config: Arc<std::sync::RwLock<CcConfig>>,
    buckets: DashMap<String, Bucket>,
    blocked: DashMap<String, Instant>,
}

impl CcEngine {
    pub fn new(config: CcConfig) -> Self {
        Self {
            config: Arc::new(std::sync::RwLock::new(config)),
            buckets: DashMap::new(),
            blocked: DashMap::new(),
        }
    }

    pub fn update_config(&self, config: CcConfig) {
        if let Ok(mut c) = self.config.write() {
            *c = config;
        }
    }

    pub fn config(&self) -> CcConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn check(&self, req: &CcCheckRequest) -> CcCheckResult {
        self.check_with_config(req, &self.config())
    }

    pub fn check_with_config(&self, req: &CcCheckRequest, cfg: &CcConfig) -> CcCheckResult {
        if !cfg.enabled {
            return CcCheckResult {
                allowed: true,
                remaining: cfg.requests_per_minute,
                retry_after_sec: None,
                reason: None,
            };
        }

        let key = format!("{}|{}|{}", req.src_ip, req.host, req.uri);

        if let Some(url) = &cfg.redis_url {
            if let Some(result) = self.check_redis(url, &key, cfg) {
                return result;
            }
        }

        if let Some(blocked_at) = self.blocked.get(&key) {
            let elapsed = blocked_at.elapsed().as_secs();
            if elapsed < cfg.block_duration_sec {
                return CcCheckResult {
                    allowed: false,
                    remaining: 0,
                    retry_after_sec: Some(cfg.block_duration_sec - elapsed),
                    reason: Some("cc_blocked".into()),
                };
            }
            self.blocked.remove(&key);
        }

        let limit = cfg.requests_per_minute.max(1) as f64;
        let burst = cfg.burst.max(1) as f64;
        let refill_per_sec = limit / 60.0;
        let now = Instant::now();

        let mut entry = self.buckets.entry(key.clone()).or_insert(Bucket {
            tokens: burst,
            last: now,
        });
        let bucket = entry.value_mut();
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill_per_sec).min(burst);
        bucket.last = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            CcCheckResult {
                allowed: true,
                remaining: bucket.tokens as u32,
                retry_after_sec: None,
                reason: None,
            }
        } else {
            self.blocked.insert(key, now);
            CcCheckResult {
                allowed: false,
                remaining: 0,
                retry_after_sec: Some(cfg.block_duration_sec.max(60)),
                reason: Some("rate_limit_exceeded".into()),
            }
        }
    }

    fn check_redis(&self, url: &str, key: &str, cfg: &CcConfig) -> Option<CcCheckResult> {
        let client = redis::Client::open(url).ok()?;
        let mut con = client.get_connection().ok()?;
        let redis_key = format!("nerve:cc:{key}");
        let count: i64 = redis::cmd("INCR").arg(&redis_key).query(&mut con).ok()?;
        if count == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(&redis_key)
                .arg(60_i64)
                .query(&mut con)
                .ok()?;
        }
        let limit = cfg.requests_per_minute as i64;
        if count > limit {
            Some(CcCheckResult {
                allowed: false,
                remaining: 0,
                retry_after_sec: Some(60),
                reason: Some("redis_rate_limit".into()),
            })
        } else {
            Some(CcCheckResult {
                allowed: true,
                remaining: (limit - count).max(0) as u32,
                retry_after_sec: None,
                reason: None,
            })
        }
    }
}

impl Default for CcEngine {
    fn default() -> Self {
        Self::new(CcConfig::default())
    }
}
