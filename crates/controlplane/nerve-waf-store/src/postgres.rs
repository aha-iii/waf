use crate::StoreError;
use nerve_waf_sdk::{EventQuery, EventQueryResult, StatsOverview, WafEventRecord};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    pub async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = crate::migrations::connect_pool(database_url).await?;
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waf_events")
            .fetch_one(&pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        tracing::info!("PostgreSQL waf_events rows={n}");
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl PostgresEventStore {
    pub async fn insert_batch(&self, events: &[WafEventRecord]) -> Result<usize, StoreError> {
        if events.is_empty() {
            return Ok(0);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        for e in events {
            let event_id = Uuid::parse_str(&e.id).unwrap_or_else(|_| Uuid::new_v4());
            sqlx::query(
                r#"INSERT INTO waf_events (
                    waf_events_id, ts, site_id, layer, action, category, rule_id, rule_name, severity,
                    src_ip, dst_ip, dst_port, method, host, uri, user_agent, headers, body,
                    payload_sample, matched_field, route_id, pod, message, trace_id, span_id, inspect_source,
                    strategy_id, strategy_name, protection_stage, rule_set, plugin_name
                ) VALUES (
                    $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,
                    $25,$26,$27,$28,$29,$30,$31
                ) ON CONFLICT (waf_events_id) DO NOTHING"#,
            )
            .bind(event_id)
            .bind(e.ts)
            .bind(&e.site_id)
            .bind(&e.layer)
            .bind(&e.action)
            .bind(&e.category)
            .bind(&e.rule_id)
            .bind(&e.rule_name)
            .bind(&e.severity)
            .bind(&e.src_ip)
            .bind(&e.dst_ip)
            .bind(e.dst_port.map(|p| p as i32))
            .bind(&e.method)
            .bind(&e.host)
            .bind(&e.uri)
            .bind(&e.user_agent)
            .bind(&e.headers)
            .bind(&e.body)
            .bind(&e.payload_sample)
            .bind(&e.matched_field)
            .bind(&e.route_id)
            .bind(&e.pod)
            .bind(&e.message)
            .bind(&e.trace_id)
            .bind(&e.span_id)
            .bind(&e.inspect_source)
            .bind(&e.strategy_id)
            .bind(&e.strategy_name)
            .bind(&e.protection_stage)
            .bind(&e.rule_set)
            .bind(&e.plugin_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(events.len())
    }

    pub async fn query(&self, q: EventQuery) -> Result<EventQueryResult, StoreError> {
        let limit = q.limit.clamp(1, 500) as i64;
        let offset = q.offset as i64;

        let total: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM waf_events
               WHERE ($1::text IS NULL OR category = $1)
                 AND ($2::text IS NULL OR action = $2)
                 AND ($3::text IS NULL OR src_ip = $3)
                 AND ($4::text IS NULL OR site_id = $4)
                 AND ($5::text IS NULL OR layer = $5)
                 AND ($6::text[] IS NULL OR cardinality($6) = 0 OR NOT (category = ANY($6)))"#,
        )
        .bind(q.category.as_deref())
        .bind(q.action.as_deref())
        .bind(q.src_ip.as_deref())
        .bind(q.site_id.as_deref())
        .bind(q.layer.as_deref())
        .bind(q.exclude_categories.as_deref())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let rows = sqlx::query(
            r#"SELECT waf_events_id, ts, site_id, layer, action, category, rule_id, rule_name, severity,
                      src_ip, dst_ip, dst_port, method, host, uri, user_agent, headers, body,
                      payload_sample, matched_field, route_id, pod, message, trace_id, span_id, inspect_source,
                      strategy_id, strategy_name, protection_stage, rule_set, plugin_name
               FROM waf_events
               WHERE ($1::text IS NULL OR category = $1)
                 AND ($2::text IS NULL OR action = $2)
                 AND ($3::text IS NULL OR src_ip = $3)
                 AND ($4::text IS NULL OR site_id = $4)
                 AND ($5::text IS NULL OR layer = $5)
                 AND ($6::text[] IS NULL OR cardinality($6) = 0 OR NOT (category = ANY($6)))
               ORDER BY ts DESC
               LIMIT $7 OFFSET $8"#,
        )
        .bind(q.category.as_deref())
        .bind(q.action.as_deref())
        .bind(q.src_ip.as_deref())
        .bind(q.site_id.as_deref())
        .bind(q.layer.as_deref())
        .bind(q.exclude_categories.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let events = rows.iter().map(row_to_event).collect();
        Ok(EventQueryResult {
            total: total as u64,
            events,
        })
    }

    pub async fn stats_overview(&self) -> Result<StatsOverview, StoreError> {
        self.stats_since(chrono::Utc::now() - chrono::Duration::hours(24))
            .await
    }

    pub async fn stats_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<StatsOverview, StoreError> {
        let total_events: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM waf_events WHERE ts >= $1")
                .bind(since)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Db(e.to_string()))?;

        let blocked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_events WHERE ts >= $1 AND action = 'block'",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let logged: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM waf_events WHERE ts >= $1 AND action = 'log'")
                .bind(since)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Db(e.to_string()))?;

        let cat_rows = sqlx::query(
            r#"SELECT category, COUNT(*) AS cnt FROM waf_events
               WHERE ts >= $1 GROUP BY category ORDER BY cnt DESC LIMIT 20"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let by_category = cat_rows
            .iter()
            .map(|r| nerve_waf_sdk::CategoryCount {
                category: r.get("category"),
                count: r.get::<i64, _>("cnt") as u64,
            })
            .collect();

        let ip_rows = sqlx::query(
            r#"SELECT src_ip, COUNT(*) AS cnt FROM waf_events
               WHERE ts >= $1 AND src_ip IS NOT NULL
               GROUP BY src_ip ORDER BY cnt DESC LIMIT 10"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let top_ips = ip_rows
            .iter()
            .map(|r| nerve_waf_sdk::IpCount {
                src_ip: r.get("src_ip"),
                count: r.get::<i64, _>("cnt") as u64,
            })
            .collect();

        Ok(StatsOverview {
            total_events: total_events as u64,
            blocked: blocked as u64,
            logged: logged as u64,
            by_category,
            top_ips,
        })
    }

    pub async fn db_status(&self) -> Result<super::DbStatus, StoreError> {
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waf_events")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let latest_ts: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT MAX(ts) FROM waf_events")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(super::DbStatus {
            backend: "postgresql",
            database: Some(database),
            event_count: event_count as u64,
            latest_ts: latest_ts.map(|t| t.to_rfc3339()),
        })
    }

    pub async fn network_stats_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
        days: i64,
    ) -> Result<serde_json::Value, StoreError> {
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM waf_events WHERE layer = 'l3' AND ts >= $1")
                .bind(since)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| StoreError::Db(e.to_string()))?;

        let blocked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_events WHERE layer = 'l3' AND ts >= $1 AND action = 'block'",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let trend_rows = sqlx::query(
            r#"SELECT to_char(date_trunc('day', ts AT TIME ZONE 'UTC'), 'YYYY-MM-DD') AS day,
                      COUNT(*)::bigint AS cnt
               FROM waf_events WHERE layer = 'l3' AND ts >= $1
               GROUP BY 1 ORDER BY 1"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        let trend: Vec<serde_json::Value> = trend_rows
            .iter()
            .map(|r| {
                json!({
                    "day": r.get::<String, _>("day"),
                    "count": r.get::<i64, _>("cnt"),
                })
            })
            .collect();

        let cat_rows = sqlx::query(
            r#"SELECT category, COUNT(*)::bigint AS cnt FROM waf_events
               WHERE layer = 'l3' AND ts >= $1 GROUP BY category ORDER BY cnt DESC LIMIT 10"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        let top_categories: Vec<serde_json::Value> = cat_rows
            .iter()
            .map(|r| {
                json!({
                    "category": r.get::<String, _>("category"),
                    "count": r.get::<i64, _>("cnt"),
                })
            })
            .collect();

        let ip_rows = sqlx::query(
            r#"SELECT COALESCE(dst_ip, src_ip) AS ip, COUNT(*)::bigint AS cnt FROM waf_events
               WHERE layer = 'l3' AND ts >= $1 AND COALESCE(dst_ip, src_ip) IS NOT NULL
               GROUP BY 1 ORDER BY cnt DESC LIMIT 10"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        let top_ips: Vec<serde_json::Value> = ip_rows
            .iter()
            .map(|r| {
                json!({
                    "ip": r.get::<String, _>("ip"),
                    "count": r.get::<i64, _>("cnt"),
                })
            })
            .collect();

        let pod_rows = sqlx::query(
            r#"SELECT pod, COUNT(*)::bigint AS cnt FROM waf_events
               WHERE layer = 'l3' AND ts >= $1 AND pod IS NOT NULL AND pod <> ''
               GROUP BY pod ORDER BY cnt DESC LIMIT 10"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        let top_pods: Vec<serde_json::Value> = pod_rows
            .iter()
            .map(|r| {
                json!({
                    "pod": r.get::<String, _>("pod"),
                    "count": r.get::<i64, _>("cnt"),
                })
            })
            .collect();

        Ok(json!({
            "total": total,
            "blocked": blocked,
            "trend": trend,
            "top_categories": top_categories,
            "top_ips": top_ips,
            "top_pods": top_pods,
            "days": days,
        }))
    }
}

fn row_to_event(r: &sqlx::postgres::PgRow) -> WafEventRecord {
    let dst_port: Option<i32> = r.get("dst_port");
    let waf_events_id: Uuid = r.get("waf_events_id");
    WafEventRecord {
        id: waf_events_id.to_string(),
        ts: r.get("ts"),
        site_id: r.get("site_id"),
        layer: r.get("layer"),
        action: r.get("action"),
        category: r.get("category"),
        rule_id: r.get("rule_id"),
        rule_name: r.get("rule_name"),
        severity: r.get("severity"),
        src_ip: r.get("src_ip"),
        dst_ip: r.get("dst_ip"),
        dst_port: dst_port.map(|p| p as u16),
        method: r.get("method"),
        host: r.get("host"),
        uri: r.get("uri"),
        user_agent: r.get("user_agent"),
        headers: r.get("headers"),
        body: r.get("body"),
        payload_sample: r.get("payload_sample"),
        matched_field: r.get("matched_field"),
        route_id: r.get("route_id"),
        pod: r.get("pod"),
        message: r.get("message"),
        trace_id: r.try_get("trace_id").ok(),
        span_id: r.try_get("span_id").ok(),
        inspect_source: r.try_get("inspect_source").ok(),
        strategy_id: r.try_get("strategy_id").ok(),
        strategy_name: r.try_get("strategy_name").ok(),
        protection_stage: r.try_get("protection_stage").ok(),
        rule_set: r.try_get("rule_set").ok(),
        plugin_name: r.try_get("plugin_name").ok(),
    }
}
