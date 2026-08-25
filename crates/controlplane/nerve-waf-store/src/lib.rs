//! Persistence for WAF config and events (JSON / Postgres).

mod file_store;
mod migrations;
mod network_stats;
pub mod ai_settings;
pub mod app_config;
pub mod app_crs;
pub mod fp_remediation;
pub mod global_config;
pub mod custom_rules;
pub mod protection_strategy;
pub mod protected_sites;
#[cfg(feature = "postgres")]
pub mod users;
#[cfg(feature = "postgres")]
pub mod demo_leads;
pub mod semantic_stats;
pub mod config;
pub mod traffic_stats;
#[cfg(feature = "postgres")]
mod topology;

#[cfg(feature = "postgres")]
mod posture;

#[cfg(feature = "postgres")]
mod ml_labels;

#[cfg(feature = "postgres")]
mod schema_check;

#[cfg(feature = "postgres")]
pub use posture::{insert_posture_scan, receet_posture_scans};

#[cfg(feature = "postgres")]
pub use ml_labels::{insert_ml_label, list_ml_labels, ml_label_stats, IesertMlLabel, MlLabelRecord};

#[cfg(feature = "postgres")]
pub use schema_check::ai_schema_status;

pub use traffic_stats::{
    blocked_attack_categories_from_events, open_traffic_store, TrafficStore,
};
#[cfg(feature = "postgres")]
pub use traffic_stats::pg_blocked_attack_categories;

#[cfg(feature = "postgres")]
pub use topology::{
    AgentRegisterRequest, FlowIngestBatch, TopologyGraph, TopologyStore, WorkloadSyncBatch,
};

#[cfg(feature = "postgres")]
mod postgres;

use chrono::Utc;
use nerve_waf_sdk::{EventQuery, EventQueryResult, StatsOverview, WafEventRecord};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbStatus {
    pub backend: &'static str,
    pub database: Option<String>,
    pub event_count: u64,
    pub latest_ts: Option<String>,
}

pub use file_store::FileEventStore;

pub use fp_remediation::{FpAnalyticsSnapshot, FpRuleCount, FpSiteCount};
#[cfg(feature = "postgres")]
pub use fp_remediation::{fp_analytics, insert_fp_audit};
pub use app_crs::{
    AppCrsBinding, AppCrsHistoryEntry, AppCrsStatus, CRS_MODULE_KEYS, SystemCrsVersionRef,
    crs_modules_snapshot, prune_disabled_rules,
};
pub use custom_rules::{CustomRulesError, CustomRulesStore, GLOBAL_CUSTOM_SITE_ID, default_custom_rules, new_rule_id, new_set_id};
pub use protection_strategy::ProtectionStrategyTemplate;
pub use protected_sites::{ProtectedSitesError, ProtectedSitesStore};
#[cfg(feature = "postgres")]
pub use users::{ConsoleUser, UsersError, UsersStore, ROLE_ADMIN, ROLE_OPERATOR, ROLE_VIEWER};
#[cfg(feature = "postgres")]
pub use demo_leads::{DemoLead, DemoLeadsError, DemoLeadsStore, NewDemoLead};
pub use ai_settings::{AiSettings, AiSettingsError, AiSettingsStore, TriageLlmConfig};
pub use semantic_stats::{SemanticStatsError, SemanticStatsStore};
pub use config::{
    open_config_backend, AuditContext, ConfigBackend, ConfigError, FileConfigStore, RuntimeDenyEntry,
};
#[cfg(feature = "postgres")]
pub use config::PostgresConfigStore;

#[cfg(feature = "postgres")]
pub use postgres::PostgresEventStore;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub enum EventStore {
    File(FileEventStore),
    #[cfg(feature = "postgres")]
    Postgres(PostgresEventStore),
}

impl EventStore {
    pub async fn insert_batch(&self, events: &[WafEventRecord]) -> Result<usize, StoreError> {
        match self {
            Self::File(s) => s.insert_batch(events).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.insert_batch(events).await,
        }
    }

    pub async fn query(&self, q: EventQuery) -> Result<EventQueryResult, StoreError> {
        match self {
            Self::File(s) => s.query(q).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.query(q).await,
        }
    }

    pub async fn stats_overview(&self) -> Result<StatsOverview, StoreError> {
        self.stats_since(Utc::now() - chrono::Duration::hours(24)).await
    }

    pub async fn stats_since(&self, since: chrono::DateTime<Utc>) -> Result<StatsOverview, StoreError> {
        match self {
            Self::File(s) => s.stats_since(since).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.stats_since(since).await,
        }
    }

    pub async fn network_stats_since(
        &self,
        since: chrono::DateTime<Utc>,
        days: i64,
    ) -> Result<serde_json::Value, StoreError> {
        match self {
            Self::File(s) => s.network_stats_since(since, days).await,
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.network_stats_since(since, days).await,
        }
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::File(_) => "file",
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgresql",
        }
    }

    /// PostgreSQL pool (PG backend only).
    #[cfg(feature = "postgres")]
    pub fn postgres_pool(&self) -> Option<sqlx::PgPool> {
        match self {
            Self::Postgres(s) => Some(s.pool().clone()),
            Self::File(_) => None,
        }
    }

    pub async fn db_status(&self) -> Result<DbStatus, StoreError> {
        match self {
            Self::File(s) => {
                let n = s
                    .query(EventQuery {
                        limit: 1,
                        offset: 0,
                        ..Default::default()
                    })
                    .await?
                    .total;
                Ok(DbStatus {
                    backend: "file",
                    database: None,
                    event_count: n,
                    latest_ts: None,
                })
            }
            #[cfg(feature = "postgres")]
            Self::Postgres(pg) => pg.db_status().await,
        }
    }
}

pub async fn open_event_store(
    database_url: Option<&str>,
    data_dir: &Path,
) -> Result<Arc<EventStore>, StoreError> {
    #[cfg(feature = "postgres")]
    if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
        let store = PostgresEventStore::connect(url).await?;
        tracing::info!("event store: PostgreSQL ({})", mask_url(url));
        return Ok(Arc::new(EventStore::Postgres(store)));
    }

    let path = data_dir.join("events.json");
    tracing::warn!(
        "DATABASE_URL 未设置，事件写入文件 {}（不会进 PostgreSQL）",
        path.display()
    );
    Ok(Arc::new(EventStore::File(FileEventStore::new(path))))
}

/// If event DB is PostgreSQL, reuse the same pool for topology store.
pub fn topology_from_events(events: &EventStore) -> Option<Arc<TopologyStore>> {
    #[cfg(feature = "postgres")]
    {
        if let EventStore::Postgres(pg) = events {
            return Some(Arc::new(TopologyStore::new(pg.pool().clone())));
        }
    }
    #[cfg(not(feature = "postgres"))]
    let _ = events;
    None
}

pub async fn open_topology_store(database_url: &str) -> Result<Arc<TopologyStore>, StoreError> {
    #[cfg(feature = "postgres")]
    {
        let pool = migrations::connect_pool(database_url).await?;
        return Ok(Arc::new(TopologyStore::new(pool)));
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = database_url;
        Err(StoreError::Db("postgres feature disabled".into()))
    }
}

fn mask_url(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(scheme) = url.find("://") {
            return format!("{}://***@{}", &url[..scheme + 3], &url[at + 1..]);
        }
    }
    "***".into()
}

#[cfg(test)]
mod test;
