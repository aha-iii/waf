//! License / 计量（Phase 6 简化）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub edition: String,
    pub max_sites: u32,
    pub max_agents: u32,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

impl Default for LicenseInfo {
    fn default() -> Self {
        Self {
            edition: "community".into(),
            max_sites: 10,
            max_agents: 50,
            expires_at: None,
        }
    }
}

pub struct UsageMeter {
    requests: AtomicU64,
    blocks: AtomicU64,
    license: std::sync::RwLock<LicenseInfo>,
}

impl UsageMeter {
    pub fn new(license: LicenseInfo) -> Self {
        Self {
            requests: AtomicU64::new(0),
            blocks: AtomicU64::new(0),
            license: std::sync::RwLock::new(license),
        }
    }

    pub fn record_request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_block(&self) {
        self.blocks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> UsageSnapshot {
        UsageSnapshot {
            requests_total: self.requests.load(Ordering::Relaxed),
            blocks_total: self.blocks.load(Ordering::Relaxed),
            license: self.license.read().unwrap().clone(),
        }
    }

    pub fn within_limits(&self, sites: u32, agents: u32) -> bool {
        let lic = self.license.read().unwrap();
        if let Some(exp) = lic.expires_at {
            if Utc::now() > exp {
                return false;
            }
        }
        sites <= lic.max_sites && agents <= lic.max_agents
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub requests_total: u64,
    pub blocks_total: u64,
    pub license: LicenseInfo,
}
