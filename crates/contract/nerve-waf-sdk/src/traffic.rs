use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One Edge batch-report delta (control plane UPSERTs into hourly buckets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficBatchItem {
    pub site_id: String,
    #[serde(default)]
    pub host: String,
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficSummary {
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
    pub block_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSeriesPoint {
    pub bucket: String,
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSiteRow {
    pub site_id: String,
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
    pub block_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficAttackCategory {
    pub category: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficHostNode {
    pub host: String,
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
    pub attack_categories: Vec<TrafficAttackCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSiteTreeNode {
    pub site_id: String,
    pub total: u64,
    pub clean: u64,
    pub observed: u64,
    pub blocked: u64,
    pub hosts: Vec<TrafficHostNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficTreeResponse {
    pub since: DateTime<Utc>,
    pub summary: TrafficSummary,
    pub sites: Vec<TrafficSiteTreeNode>,
}

impl TrafficSummary {
    pub fn from_counts(total: u64, clean: u64, observed: u64, blocked: u64) -> Self {
        let block_rate = if total > 0 {
            (blocked as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        Self {
            total,
            clean,
            observed,
            blocked,
            block_rate,
        }
    }
}
