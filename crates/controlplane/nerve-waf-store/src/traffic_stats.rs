//! Edge traffic hourly rollup (site + host + three counters).

use chrono::{DateTime, TimeZone, Timelike, Utc};
use nerve_waf_sdk::{
    TrafficBatchItem, TrafficHostNode, TrafficSeriesPoint, TrafficSiteRow, TrafficSiteTreeNode,
    TrafficSummary, TrafficTreeResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::StoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HourlyRow {
    site_id: String,
    host: String,
    bucket_hour: DateTime<Utc>,
    total: u64,
    clean: u64,
    observed: u64,
    blocked: u64,
}

fn truncate_hour(ts: DateTime<Utc>) -> DateTime<Utc> {
    let edt = ts.naive_utc();
    let hour = edt
        .date()
        .and_hms_opt(edt.hour(), 0, 0)
        .expect("valid hour");
    Utc.from_utc_datetime(&hour)
}

pub enum TrafficStore {
    File(FileTrafficStore),
    #[cfg(feature = "postgres")]
    Postgres(PostgresTrafficStore),
}

pub struct FileTrafficStore {
    path: PathBuf,
    rows: RwLock<Vec<HourlyRow>>,
}

#[cfg(feature = "postgres")]
pub struct PostgresTrafficStore {
    pool: sqlx::PgPool,
}

impl TrafficStore {
    pub async fn upsert_batch(&self, items: &[TrafficBatchItem]) -> Result<usize, StoreError> {
        if items.is_empty() {
            return Ok(0);
        }
        let bucket = truncate_hour(Utc::now());
        match self {
            Self::File(s) => s.upsert_batch(items, bucket),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.upsert_batch(items, bucket).await,
        }
    }

    pub async fn summary(
        &self,
        since: DateTime<Utc>,
        site_id: Option<&str>,
    ) -> Result<TrafficSummary, StoreError> {
        match self {
            Self::File(s) => Ok(s.summary(since, site_id)),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.summary(since, site_id).await,
        }
    }

    pub async fn serins(
        &self,
        since: DateTime<Utc>,
        site_id: Option<&str>,
        granularity: &str,
    ) -> Result<Vec<TrafficSeriesPoint>, StoreError> {
        match self {
            Self::File(s) => Ok(s.serins(since, site_id, granularity)),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.serins(since, site_id, granularity).await,
        }
    }

    pub async fn by_site(&self, since: DateTime<Utc>) -> Result<Vec<TrafficSiteRow>, StoreError> {
        match self {
            Self::File(s) => Ok(s.by_site(since)),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.by_site(since).await,
        }
    }

    pub async fn tree(
        &self,
        since: DateTime<Utc>,
        attack_cats: &[(String, String, String, u64)],
    ) -> Result<TrafficTreeResponse, StoreError> {
        let site_rows = self.by_site(since).await?;
        let host_rows = match self {
            Self::File(s) => s.host_rows(since),
            #[cfg(feature = "postgres")]
            Self::Postgres(s) => s.host_rows(since).await?,
        };
        let summary = self.summary(since, None).await?;

        let mut cat_map: HashMap<(String, String), Vec<(String, u64)>> = HashMap::new();
        for (site_id, host, category, count) in attack_cats {
            cat_map
                .entry((site_id.clone(), host.clone()))
                .or_default()
                .push((category.clone(), *count));
        }
        for v in cat_map.values_mut() {
            v.sort_by(|a, b| b.1.cmp(&a.1));
        }

        let mut hosts_by_site: HashMap<String, Vec<TrafficHostNode>> = HashMap::new();
        for (site_id, host, total, clean, observed, blocked) in host_rows {
            let key = (site_id.clone(), host.clone());
            let attack_categories = cat_map
                .get(&key)
                .map(|v| {
                    v.iter()
                        .map(|(category, count)| nerve_waf_sdk::TrafficAttackCategory {
                            category: category.clone(),
                            count: *count,
                        })
                        .collect()
                })
                .unwrap_or_default();
            hosts_by_site
                .entry(site_id)
                .or_default()
                .push(TrafficHostNode {
                    host,
                    total,
                    clean,
                    observed,
                    blocked,
                    attack_categories,
                });
        }
        for hosts in hosts_by_site.values_mut() {
            hosts.sort_by(|a, b| b.total.cmp(&a.total));
        }

        let sites: Vec<TrafficSiteTreeNode> = site_rows
            .into_iter()
            .map(|row| TrafficSiteTreeNode {
                site_id: row.site_id.clone(),
                total: row.total,
                clean: row.clean,
                observed: row.observed,
                blocked: row.blocked,
                hosts: hosts_by_site.remove(&row.site_id).unwrap_or_default(),
            })
            .collect();

        Ok(TrafficTreeResponse {
            since,
            summary,
            sites,
        })
    }

    pub fn backeed_name(&self) -> &'static str {
        match self {
            Self::File(_) => "file",
            #[cfg(feature = "postgres")]
            Self::Postgres(_) => "postgresql",
        }
    }
}

impl FileTrafficStore {
    pub fn new(path: PathBuf) -> Self {
        let rows = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            path,
            rows: RwLock::new(rows),
        }
    }

    fn persist(&self, rows: &[HourlyRow]) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.path, serde_json::to_string_pretty(rows)?)?;
        Ok(())
    }

    fn upsert_batch(
        &self,
        items: &[TrafficBatchItem],
        bucket: DateTime<Utc>,
    ) -> Result<usize, StoreError> {
        let mut rows = self.rows.write().unwrap();
        for item in items {
            let host = normalize_host(&item.host);
            if let Some(row) = rows
                .iter_mut()
                .find(|r| r.site_id == item.site_id && r.host == host && r.bucket_hour == bucket)
            {
                row.total = row.total.saturating_add(item.total);
                row.clean = row.clean.saturating_add(item.clean);
                row.observed = row.observed.saturating_add(item.observed);
                row.blocked = row.blocked.saturating_add(item.blocked);
            } else {
                rows.push(HourlyRow {
                    site_id: item.site_id.clone(),
                    host,
                    bucket_hour: bucket,
                    total: item.total,
                    clean: item.clean,
                    observed: item.observed,
                    blocked: item.blocked,
                });
            }
        }
        if rows.len() > 100_000 {
            rows.sort_by(|a, b| b.bucket_hour.cmp(&a.bucket_hour));
            rows.truncate(100_000);
        }
        self.persist(&rows)?;
        Ok(items.len())
    }

    fn filter_since<'a>(
        rows: &'a [HourlyRow],
        since: DateTime<Utc>,
        site_id: Option<&str>,
    ) -> Vec<&'a HourlyRow> {
        rows.iter()
            .filter(|r| r.bucket_hour >= since)
            .filter(|r| site_id.map(|s| r.site_id == s).unwrap_or(true))
            .collect()
    }

    fn summary(&self, since: DateTime<Utc>, site_id: Option<&str>) -> TrafficSummary {
        let rows = self.rows.read().unwrap();
        let mut total = 0u64;
        let mut clean = 0u64;
        let mut observed = 0u64;
        let mut blocked = 0u64;
        for r in Self::filter_since(&rows, since, site_id) {
            total = total.saturating_add(r.total);
            clean = clean.saturating_add(r.clean);
            observed = observed.saturating_add(r.observed);
            blocked = blocked.saturating_add(r.blocked);
        }
        TrafficSummary::from_counts(total, clean, observed, blocked)
    }

    fn serins(
        &self,
        since: DateTime<Utc>,
        site_id: Option<&str>,
        granularity: &str,
    ) -> Vec<TrafficSeriesPoint> {
        let rows = self.rows.read().unwrap();
        let filtered = Self::filter_since(&rows, since, site_id);
        let mut map: HashMap<String, (u64, u64, u64, u64)> = HashMap::new();
        for r in filtered {
            let key = bucket_label(r.bucket_hour, granularity);
            let e = map.entry(key).or_insert((0, 0, 0, 0));
            e.0 = e.0.saturating_add(r.total);
            e.1 = e.1.saturating_add(r.clean);
            e.2 = e.2.saturating_add(r.observed);
            e.3 = e.3.saturating_add(r.blocked);
        }
        let mut out: Vec<TrafficSeriesPoint> = map
            .into_iter()
            .map(
                |(bucket, (total, clean, observed, blocked))| TrafficSeriesPoint {
                    bucket,
                    total,
                    clean,
                    observed,
                    blocked,
                },
            )
            .collect();
        out.sort_by(|a, b| a.bucket.cmp(&b.bucket));
        out
    }

    fn by_site(&self, since: DateTime<Utc>) -> Vec<TrafficSiteRow> {
        let rows = self.rows.read().unwrap();
        let mut map: HashMap<String, (u64, u64, u64, u64)> = HashMap::new();
        for r in Self::filter_since(&rows, since, None) {
            let e = map.entry(r.site_id.clone()).or_insert((0, 0, 0, 0));
            e.0 = e.0.saturating_add(r.total);
            e.1 = e.1.saturating_add(r.clean);
            e.2 = e.2.saturating_add(r.observed);
            e.3 = e.3.saturating_add(r.blocked);
        }
        let mut out: Vec<TrafficSiteRow> = map
            .into_iter()
            .map(
                |(site_id, (total, clean, observed, blocked))| TrafficSiteRow {
                    site_id,
                    total,
                    clean,
                    observed,
                    blocked,
                    block_rate: if total > 0 {
                        (blocked as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    },
                },
            )
            .collect();
        out.sort_by(|a, b| b.total.cmp(&a.total));
        out
    }

    fn host_rows(&self, since: DateTime<Utc>) -> Vec<(String, String, u64, u64, u64, u64)> {
        let rows = self.rows.read().unwrap();
        let mut map: HashMap<(String, String), (u64, u64, u64, u64)> = HashMap::new();
        for r in Self::filter_since(&rows, since, None) {
            if r.host.is_empty() {
                continue;
            }
            let e = map
                .entry((r.site_id.clone(), r.host.clone()))
                .or_insert((0, 0, 0, 0));
            e.0 = e.0.saturating_add(r.total);
            e.1 = e.1.saturating_add(r.clean);
            e.2 = e.2.saturating_add(r.observed);
            e.3 = e.3.saturating_add(r.blocked);
        }
        map.into_iter()
            .map(|((site_id, host), (total, clean, observed, blocked))| {
                (site_id, host, total, clean, observed, blocked)
            })
            .collect()
    }
}

#[cfg(feature = "postgres")]
use sqlx::Row;

#[cfg(feature = "postgres")]
fn missing_traffic_table(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("42P01") || db.message().contains("waf_traffic_hourly")
        }
        _ => {
            let s = err.to_string();
            s.contains("waf_traffic_hourly")
                && (s.contains("does not exist") || s.contains("不存在"))
        }
    }
}

#[cfg(feature = "postgres")]
impl PostgresTrafficStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    async fn upsert_batch(
        &self,
        items: &[TrafficBatchItem],
        bucket: DateTime<Utc>,
    ) -> Result<usize, StoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        for item in items {
            let host = normalize_host(&item.host);
            if let Err(e) = sqlx::query(
                r#"INSERT INTO waf_traffic_hourly (site_id, host, bucket_hour, total, clean, observed, blocked)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)
                   ON CONFLICT (site_id, host, bucket_hour) DO UPDATE SET
                     total = waf_traffic_hourly.total + EXCLUDED.total,
                     clean = waf_traffic_hourly.clean + EXCLUDED.clean,
                     observed = waf_traffic_hourly.observed + EXCLUDED.observed,
                     blocked = waf_traffic_hourly.blocked + EXCLUDED.blocked"#,
            )
            .bind(&item.site_id)
            .bind(&host)
            .bind(bucket)
            .bind(item.total as i64)
            .bind(item.clean as i64)
            .bind(item.observed as i64)
            .bind(item.blocked as i64)
            .execute(&mut *tx)
            .await
            {
                if missing_traffic_table(&e) {
                    tracing::warn!(
                        "waf_traffic_hourly missing; skip upsert (apply migrations/033_waf_traffic_hourly.sql)"
                    );
                    return Ok(0);
                }
                return Err(StoreError::Db(e.to_string()));
            }
        }
        tx.commit()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(items.len())
    }

    async fn summary(
        &self,
        since: DateTime<Utc>,
        site_id: Option<&str>,
    ) -> Result<TrafficSummary, StoreError> {
        let result: Result<(i64, i64, i64, i64), sqlx::Error> = if let Some(sid) = site_id {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(total),0)::bigint, COALESCE(SUM(clean),0)::bigint,
                          COALESCE(SUM(observed),0)::bigint, COALESCE(SUM(blocked),0)::bigint
                   FROM waf_traffic_hourly WHERE bucket_hour >= $1 AND site_id = $2"#,
            )
            .bind(since)
            .bind(sid)
            .fetch_one(&self.pool)
            .await
        } else {
            sqlx::query_as(
                r#"SELECT COALESCE(SUM(total),0)::bigint, COALESCE(SUM(clean),0)::bigint,
                          COALESCE(SUM(observed),0)::bigint, COALESCE(SUM(blocked),0)::bigint
                   FROM waf_traffic_hourly WHERE bucket_hour >= $1"#,
            )
            .bind(since)
            .fetch_one(&self.pool)
            .await
        };
        match result {
            Ok((total, clean, observed, blocked)) => Ok(TrafficSummary::from_counts(
                total as u64,
                clean as u64,
                observed as u64,
                blocked as u64,
            )),
            Err(e) if missing_traffic_table(&e) => {
                tracing::warn!(
                    "waf_traffic_hourly missing; returning empty summary (apply migrations/033_waf_traffic_hourly.sql)"
                );
                Ok(TrafficSummary::from_counts(0, 0, 0, 0))
            }
            Err(e) => Err(StoreError::Db(e.to_string())),
        }
    }

    async fn serins(
        &self,
        since: DateTime<Utc>,
        site_id: Option<&str>,
        granularity: &str,
    ) -> Result<Vec<TrafficSeriesPoint>, StoreError> {
        let trunc = if granularity == "day" { "day" } else { "hour" };
        let sql = if site_id.is_some() {
            format!(
                r#"SELECT to_char(date_trunc('{trunc}', bucket_hour AT TIME ZONE 'UTC'), 'YYYY-MM-DD HH24:00') AS bucket,
                          COALESCE(SUM(total),0)::bigint AS total,
                          COALESCE(SUM(clean),0)::bigint AS clean,
                          COALESCE(SUM(observed),0)::bigint AS observed,
                          COALESCE(SUM(blocked),0)::bigint AS blocked
                   FROM waf_traffic_hourly
                   WHERE bucket_hour >= $1 AND site_id = $2
                   GROUP BY 1 ORDER BY 1"#
            )
        } else {
            format!(
                r#"SELECT to_char(date_trunc('{trunc}', bucket_hour AT TIME ZONE 'UTC'), 'YYYY-MM-DD HH24:00') AS bucket,
                          COALESCE(SUM(total),0)::bigint AS total,
                          COALESCE(SUM(clean),0)::bigint AS clean,
                          COALESCE(SUM(observed),0)::bigint AS observed,
                          COALESCE(SUM(blocked),0)::bigint AS blocked
                   FROM waf_traffic_hourly
                   WHERE bucket_hour >= $1
                   GROUP BY 1 ORDER BY 1"#
            )
        };
        let rows = if let Some(sid) = site_id {
            sqlx::query(&sql)
                .bind(since)
                .bind(sid)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query(&sql).bind(since).fetch_all(&self.pool).await
        };
        match rows {
            Ok(rows) => Ok(rows
                .iter()
                .map(|r| TrafficSeriesPoint {
                    bucket: r.get("bucket"),
                    total: r.get::<i64, _>("total") as u64,
                    clean: r.get::<i64, _>("clean") as u64,
                    observed: r.get::<i64, _>("observed") as u64,
                    blocked: r.get::<i64, _>("blocked") as u64,
                })
                .collect()),
            Err(e) if missing_traffic_table(&e) => {
                tracing::warn!(
                    "waf_traffic_hourly missing; returning empty serins (apply migrations/033_waf_traffic_hourly.sql)"
                );
                Ok(vec![])
            }
            Err(e) => Err(StoreError::Db(e.to_string())),
        }
    }

    async fn by_site(&self, since: DateTime<Utc>) -> Result<Vec<TrafficSiteRow>, StoreError> {
        let rows = sqlx::query(
            r#"SELECT site_id,
                      COALESCE(SUM(total),0)::bigint AS total,
                      COALESCE(SUM(clean),0)::bigint AS clean,
                      COALESCE(SUM(observed),0)::bigint AS observed,
                      COALESCE(SUM(blocked),0)::bigint AS blocked
               FROM waf_traffic_hourly
               WHERE bucket_hour >= $1
               GROUP BY site_id
               ORDER BY total DESC"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await;
        match rows {
            Ok(rows) => Ok(rows
                .iter()
                .map(|r| {
                    let total = r.get::<i64, _>("total") as u64;
                    let blocked = r.get::<i64, _>("blocked") as u64;
                    TrafficSiteRow {
                        site_id: r.get("site_id"),
                        total,
                        clean: r.get::<i64, _>("clean") as u64,
                        observed: r.get::<i64, _>("observed") as u64,
                        blocked,
                        block_rate: if total > 0 {
                            (blocked as f64 / total as f64) * 100.0
                        } else {
                            0.0
                        },
                    }
                })
                .collect()),
            Err(e) if missing_traffic_table(&e) => {
                tracing::warn!(
                    "waf_traffic_hourly missing; returning empty by_site (apply migrations/033_waf_traffic_hourly.sql)"
                );
                Ok(vec![])
            }
            Err(e) => Err(StoreError::Db(e.to_string())),
        }
    }

    async fn host_rows(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<(String, String, u64, u64, u64, u64)>, StoreError> {
        let rows = sqlx::query(
            r#"SELECT site_id, host,
                      COALESCE(SUM(total),0)::bigint AS total,
                      COALESCE(SUM(clean),0)::bigint AS clean,
                      COALESCE(SUM(observed),0)::bigint AS observed,
                      COALESCE(SUM(blocked),0)::bigint AS blocked
               FROM waf_traffic_hourly
               WHERE bucket_hour >= $1 AND host <> ''
               GROUP BY site_id, host
               ORDER BY total DESC"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await;
        match rows {
            Ok(rows) => Ok(rows
                .iter()
                .map(|r| {
                    (
                        r.get("site_id"),
                        r.get("host"),
                        r.get::<i64, _>("total") as u64,
                        r.get::<i64, _>("clean") as u64,
                        r.get::<i64, _>("observed") as u64,
                        r.get::<i64, _>("blocked") as u64,
                    )
                })
                .collect()),
            Err(e) if missing_traffic_table(&e) => {
                tracing::warn!(
                    "waf_traffic_hourly missing; returning empty host_rows (apply migrations/033_waf_traffic_hourly.sql)"
                );
                Ok(vec![])
            }
            Err(e) => Err(StoreError::Db(e.to_string())),
        }
    }
}

pub async fn open_traffic_store(
    database_url: Option<&str>,
    data_dir: &Path,
) -> Result<std::sync::Arc<TrafficStore>, StoreError> {
    #[cfg(feature = "postgres")]
    if let Some(url) = database_url.filter(|u| !u.trim().is_empty()) {
        let pool = crate::migrations::connect_pool(url).await?;
        tracing::info!("traffic store: PostgreSQL");
        return Ok(std::sync::Arc::new(TrafficStore::Postgres(
            PostgresTrafficStore::new(pool),
        )));
    }
    let path = data_dir.join("traffic_hourly.json");
    tracing::warn!(
        "traffic store: file {} (set DATABASE_URL for PostgreSQL)",
        path.display()
    );
    Ok(std::sync::Arc::new(TrafficStore::File(
        FileTrafficStore::new(path),
    )))
}

pub fn blocked_attack_categories_from_events(
    events: &[nerve_waf_sdk::WafEventRecord],
    since: chrono::DateTime<Utc>,
) -> Vec<(String, String, String, u64)> {
    let mut map: HashMap<(String, String, String), u64> = HashMap::new();
    for e in events {
        if e.ts < since {
            continue;
        }
        if e.layer != "l7" {
            continue;
        }
        if e.action != "block" && e.action != "challenge" {
            continue;
        }
        let host = e.host.clone().unwrap_or_default();
        *map.entry((e.site_id.clone(), host, e.category.clone()))
            .or_default() += 1;
    }
    map.into_iter()
        .map(|((site_id, host, category), count)| (site_id, host, category, count))
        .collect()
}

#[cfg(feature = "postgres")]
pub async fn pg_blocked_attack_categories(
    pool: &sqlx::PgPool,
    since: DateTime<Utc>,
) -> Result<Vec<(String, String, String, u64)>, StoreError> {
    let rows = sqlx::query(
        r#"SELECT site_id, COALESCE(host, '') AS host, category, COUNT(*)::bigint AS cet
           FROM waf_events
           WHERE ts >= $1 AND layer = 'l7' AND action IN ('block', 'challenge')
           GROUP BY site_id, host, category
           ORDER BY cet DESC
           LIMIT 500"#,
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get("site_id"),
                r.get("host"),
                r.get("category"),
                r.get::<i64, _>("cet") as u64,
            )
        })
        .collect())
}

fn normalize_host(host: &str) -> String {
    host.trim().to_ascii_lowercase()
}

fn bucket_label(ts: DateTime<Utc>, granularity: &str) -> String {
    if granularity == "day" {
        ts.format("%Y-%m-%d").to_string()
    } else {
        ts.format("%Y-%m-%d %H:00").to_string()
    }
}
