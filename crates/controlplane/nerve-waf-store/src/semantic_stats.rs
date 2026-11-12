use nerve_waf_semantic::{
    InMemorySemanticStats, SemanticCounters, SemanticFeedback, SemanticSample,
    SemanticStatsRecorder, SemanticStatsSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemanticStatsError {
    #[error("database: {0}")]
    Db(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SitePersisted {
    counters: SemanticCounters,
    samples: Vec<SemanticSample>,
    feedback: Vec<SemanticFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StatsFileFormat {
    Legacy(SitePersisted),
    Multi {
        sites: HashMap<String, SitePersisted>,
    },
}

pub struct SemanticStatsStore {
    inner: InMemorySemanticStats,
    file_path: PathBuf,
    #[cfg(feature = "postgres")]
    pool: Option<sqlx::PgPool>,
    dirty: Mutex<HashSet<String>>,
}

impl SemanticStatsStore {
    pub async fn open(
        database_url: Option<&str>,
        data_dir: &Path,
    ) -> Result<Self, SemanticStatsError> {
        let file_path = data_dir.join("semantic_stats.json");
        #[cfg(feature = "postgres")]
        let pool = if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
            Some(
                crate::migrations::connect_pool(url)
                    .await
                    .map_err(|e| SemanticStatsError::Db(e.to_string()))?,
            )
        } else {
            None
        };
        #[cfg(not(feature = "postgres"))]
        let _ = database_url;

        let store = Self {
            inner: InMemorySemanticStats::new(),
            file_path,
            #[cfg(feature = "postgres")]
            pool,
            dirty: Mutex::new(HashSet::new()),
        };
        store.load_all_into_memory().await?;
        Ok(store)
    }

    pub fn backend_name(&self) -> &'static str {
        #[cfg(feature = "postgres")]
        if self.pool.is_some() {
            return "postgresql";
        }
        "file"
    }

    pub fn spawn_flush_task(self: std::sync::Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Err(e) = self.flush_dirty().await {
                    tracing::warn!("semantic stats flush: {e}");
                }
            }
        });
    }

    fn mark_dirty(&self, site_id: &str) {
        if let Ok(mut d) = self.dirty.lock() {
            d.insert(site_id.to_string());
        }
    }

    async fn load_all_into_memory(&self) -> Result<(), SemanticStatsError> {
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            self.load_from_pg(pool).await?;
            return Ok(());
        }
        self.load_from_file()
    }

    fn load_from_file(&self) -> Result<(), SemanticStatsError> {
        if !self.file_path.exists() {
            return Ok(());
        }
        let raw = std::fs::read_to_string(&self.file_path)?;
        let map = parse_stats_file(&raw)?;
        for (site_id, data) in map {
            self.hydrate_site(&site_id, data);
        }
        Ok(())
    }

    fn hydrate_site(&self, site_id: &str, data: SitePersisted) {
        self.inner
            .load_site(site_id, data.counters, data.samples, data.feedback);
    }

    #[cfg(feature = "postgres")]
    async fn load_from_pg(&self, pool: &sqlx::PgPool) -> Result<(), SemanticStatsError> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT site_id, counters FROM waf_semantic_counters")
            .fetch_all(pool)
            .await
            .map_err(|e| SemanticStatsError::Db(e.to_string()))?;
        for row in rows {
            let site_id: String = row.get("site_id");
            let val: serde_json::Value = row.get("counters");
            let counters: SemanticCounters = serde_json::from_value(val).unwrap_or_default();
            let samples = self.load_samples_pg(pool, &site_id).await?;
            let feedback = self.load_feedback_pg(pool, &site_id).await?;
            self.hydrate_site(
                &site_id,
                SitePersisted {
                    counters,
                    samples,
                    feedback,
                },
            );
        }
        Ok(())
    }

    #[cfg(feature = "postgres")]
    async fn load_samples_pg(
        &self,
        pool: &sqlx::PgPool,
        site_id: &str,
    ) -> Result<Vec<SemanticSample>, SemanticStatsError> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"SELECT EXTRACT(EPOCH FROM ts)::bigint AS ts,
                      param_path, decode_chain, semantic_score, engines, uri, hit
               FROM waf_semantic_samples WHERE site_id = $1
               ORDER BY ts DESC LIMIT 50"#,
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| SemanticStatsError::Db(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| SemanticSample {
                ts: r.get::<i64, _>("ts").to_string(),
                param_path: r.get("param_path"),
                decode_chain: serde_json::from_value(r.get("decode_chain")).unwrap_or_default(),
                semantic_score: r.get::<i16, _>("semantic_score") as u8,
                engines: serde_json::from_value(r.get("engines")).unwrap_or_default(),
                uri: r.get("uri"),
                hit: r.get("hit"),
            })
            .collect())
    }

    #[cfg(feature = "postgres")]
    async fn load_feedback_pg(
        &self,
        pool: &sqlx::PgPool,
        site_id: &str,
    ) -> Result<Vec<SemanticFeedback>, SemanticStatsError> {
        use sqlx::Row;
        let rows = sqlx::query(
            r#"SELECT EXTRACT(EPOCH FROM ts)::bigint AS ts, event_id, verdict, note,
                      param_path, uri, applied_action
               FROM waf_semantic_feedback WHERE site_id = $1
               ORDER BY ts DESC LIMIT 100"#,
        )
        .bind(site_id)
        .fetch_all(pool)
        .await
        .map_err(|e| SemanticStatsError::Db(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| SemanticFeedback {
                ts: r.get::<i64, _>("ts").to_string(),
                site_id: site_id.to_string(),
                event_id: r.get("event_id"),
                verdict: r.get("verdict"),
                note: r.get("note"),
                param_path: r.get("param_path"),
                uri: r.get("uri"),
                applied_action: r.get("applied_action"),
            })
            .collect())
    }

    pub async fn flush_dirty(&self) -> Result<(), SemanticStatsError> {
        let sites: Vec<String> = self
            .dirty
            .lock()
            .map(|mut d| {
                let keys: Vec<_> = d.drain().collect();
                keys
            })
            .unwrap_or_default();
        for site_id in sites {
            self.persist_site(&site_id).await?;
        }
        Ok(())
    }

    async fn persist_site(&self, site_id: &str) -> Result<(), SemanticStatsError> {
        let snap = self.inner.snapshot(site_id);
        let data = SitePersisted {
            counters: snap.counters,
            samples: snap.recent_samples,
            feedback: snap.feedback,
        };
        #[cfg(feature = "postgres")]
        if let Some(pool) = &self.pool {
            self.save_site_pg(pool, site_id, &data).await?;
        }
        self.save_site_file(site_id, &data)?;
        Ok(())
    }

    fn save_site_file(
        &self,
        site_id: &str,
        data: &SitePersisted,
    ) -> Result<(), SemanticStatsError> {
        let mut map = if self.file_path.exists() {
            let raw = std::fs::read_to_string(&self.file_path)?;
            parse_stats_file(&raw)?
        } else {
            HashMap::new()
        };
        map.insert(site_id.to_string(), data.clone());
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let wrapped = StatsFileFormat::Multi { sites: map };
        std::fs::write(&self.file_path, serde_json::to_string_pretty(&wrapped)?)?;
        Ok(())
    }

    #[cfg(feature = "postgres")]
    async fn save_site_pg(
        &self,
        pool: &sqlx::PgPool,
        site_id: &str,
        data: &SitePersisted,
    ) -> Result<(), SemanticStatsError> {
        let counters = serde_json::to_value(&data.counters)
            .map_err(|e| SemanticStatsError::Db(e.to_string()))?;
        sqlx::query(
            r#"INSERT INTO waf_semantic_counters (site_id, counters, updated_at)
               VALUES ($1, $2, now())
               ON CONFLICT (site_id) DO UPDATE SET counters = EXCLUDED.counters, updated_at = now()"#,
        )
        .bind(site_id)
        .bind(counters)
        .execute(pool)
        .await
        .map_err(|e| SemanticStatsError::Db(e.to_string()))?;
        Ok(())
    }

    #[cfg(feature = "postgres")]
    fn spawn_sample_insert(&self, site_id: &str, sample: &SemanticSample) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let site_id = site_id.to_string();
        let sample = sample.clone();
        handle.spawn(async move {
            let decode = serde_json::to_value(&sample.decode_chain).unwrap_or_default();
            let engines = serde_json::to_value(&sample.engines).unwrap_or_default();
            let ts = sample.ts.parse::<i64>().unwrap_or(0);
            let _ = sqlx::query(
                r#"INSERT INTO waf_semantic_samples
                   (site_id, ts, param_path, decode_chain, semantic_score, engines, uri, hit)
                   VALUES ($1, to_timestamp($2), $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(&site_id)
            .bind(ts)
            .bind(&sample.param_path)
            .bind(decode)
            .bind(i16::from(sample.semantic_score))
            .bind(engines)
            .bind(&sample.uri)
            .bind(sample.hit)
            .execute(&pool)
            .await;
            let _ = sqlx::query(
                "DELETE FROM waf_semantic_samples WHERE id IN (
                    SELECT id FROM waf_semantic_samples WHERE site_id = $1
                    ORDER BY ts DESC OFFSET 50
                 )",
            )
            .bind(&site_id)
            .execute(&pool)
            .await;
        });
    }

    #[cfg(feature = "postgres")]
    fn spawn_feedback_insert(&self, fb: &SemanticFeedback) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let fb = fb.clone();
        handle.spawn(async move {
            let ts = fb.ts.parse::<i64>().unwrap_or(0);
            let _ = sqlx::query(
                r#"INSERT INTO waf_semantic_feedback
                   (site_id, ts, event_id, verdict, note, param_path, uri, applied_action)
                   VALUES ($1, to_timestamp($2), $3, $4, $5, $6, $7, $8)"#,
            )
            .bind(&fb.site_id)
            .bind(ts)
            .bind(&fb.event_id)
            .bind(&fb.verdict)
            .bind(&fb.note)
            .bind(&fb.param_path)
            .bind(&fb.uri)
            .bind(&fb.applied_action)
            .execute(&pool)
            .await;
        });
    }
}

fn parse_stats_file(raw: &str) -> Result<HashMap<String, SitePersisted>, SemanticStatsError> {
    let parsed: StatsFileFormat = serde_json::from_str(raw)?;
    Ok(match parsed {
        StatsFileFormat::Legacy(data) => {
            let mut m = HashMap::new();
            m.insert("default".into(), data);
            m
        }
        StatsFileFormat::Multi { sites } => sites,
    })
}

impl SemanticStatsRecorder for SemanticStatsStore {
    fn record_inspect(&self, site_id: &str) {
        self.inner.record_inspect(site_id);
        self.mark_dirty(site_id);
    }

    fn record_hit(&self, site_id: &str, sample: SemanticSample) {
        #[cfg(feature = "postgres")]
        self.spawn_sample_insert(site_id, &sample);
        self.inner.record_hit(site_id, sample);
        self.mark_dirty(site_id);
    }

    fn record_miss(&self, site_id: &str, sample: Option<SemanticSample>) {
        if let Some(ref s) = sample {
            #[cfg(feature = "postgres")]
            self.spawn_sample_insert(site_id, s);
        }
        self.inner.record_miss(site_id, sample);
        self.mark_dirty(site_id);
    }

    fn record_whitelist_skip(&self, site_id: &str) {
        self.inner.record_whitelist_skip(site_id);
        self.mark_dirty(site_id);
    }

    fn record_timeout(&self, site_id: &str) {
        self.inner.record_timeout(site_id);
        self.mark_dirty(site_id);
    }

    fn record_slow_path(&self, site_id: &str, ran: bool) {
        self.inner.record_slow_path(site_id, ran);
        self.mark_dirty(site_id);
    }

    fn add_feedback(&self, fb: SemanticFeedback) {
        #[cfg(feature = "postgres")]
        self.spawn_feedback_insert(&fb);
        let site_id = fb.site_id.clone();
        self.inner.add_feedback(fb);
        self.mark_dirty(&site_id);
    }

    fn feedback_for_site(&self, site_id: &str) -> Vec<SemanticFeedback> {
        self.inner.feedback_for_site(site_id)
    }

    fn snapshot(&self, site_id: &str) -> SemanticStatsSnapshot {
        let mut snap = self.inner.snapshot(site_id);
        snap.backend = self.backend_name();
        snap
    }

    fn backend_name(&self) -> &'static str {
        #[cfg(feature = "postgres")]
        if self.pool.is_some() {
            return "postgresql";
        }
        "file"
    }
}
