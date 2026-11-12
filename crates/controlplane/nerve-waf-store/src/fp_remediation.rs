#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
use serde_json::Value;
#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};

use crate::StoreError;

// Audit insert mirrors the fp_audit table columns 1:1.
#[cfg(feature = "postgres")]
#[allow(clippy::too_many_arguments)]
pub async fn insert_fp_audit(
    pool: &PgPool,
    audit_id: &str,
    event_id: &str,
    site_id: &str,
    src_ip: Option<&str>,
    uri: Option<&str>,
    rule_id: Option<&str>,
    actions: &Value,
    note: Option<&str>,
    operator: &str,
) -> Result<(), StoreError> {
    let audit_uuid = uuid::Uuid::parse_str(audit_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
    let event_uuid = uuid::Uuid::parse_str(event_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
    sqlx::query(
        r#"INSERT INTO waf_fp_remediation
           (waf_fp_remediation_id, event_id, site_id, src_ip, uri, rule_id, actions, note, operator)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(audit_uuid)
    .bind(event_uuid)
    .bind(site_id)
    .bind(src_ip)
    .bind(uri)
    .bind(rule_id)
    .bind(actions)
    .bind(note)
    .bind(operator)
    .execute(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct FpAnalyticsSnapshot {
    pub confirmed_fp: i64,
    pub top_rules: Vec<FpRuleCount>,
    pub by_site: Vec<FpSiteCount>,
}

#[derive(Debug, serde::Serialize)]
pub struct FpRuleCount {
    pub rule_id: String,
    pub count: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct FpSiteCount {
    pub site_id: String,
    pub count: i64,
}

#[cfg(feature = "postgres")]
pub async fn fp_analytics(
    pool: &PgPool,
    since: DateTime<Utc>,
    site_id: Option<&str>,
) -> Result<FpAnalyticsSnapshot, StoreError> {
    let confirmed_fp = if let Some(site) = site_id {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_fp_remediation WHERE site_id = $1 AND created_at >= $2",
        )
        .bind(site)
        .bind(since)
        .fetch_one(pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM waf_fp_remediation WHERE created_at >= $1")
            .bind(since)
            .fetch_one(pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
    };

    let rule_rows = sqlx::query(
        r#"SELECT rule_id, COUNT(*)::bigint AS cnt
           FROM waf_fp_remediation
           WHERE created_at >= $1
             AND ($2::text IS NULL OR site_id = $2)
             AND rule_id IS NOT NULL AND rule_id <> ''
           GROUP BY rule_id
           ORDER BY cnt DESC
           LIMIT 10"#,
    )
    .bind(since)
    .bind(site_id)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;

    let top_rules = rule_rows
        .iter()
        .map(|r| FpRuleCount {
            rule_id: r.try_get("rule_id").unwrap_or_default(),
            count: r.try_get("cnt").unwrap_or(0),
        })
        .collect();

    let site_rows = sqlx::query(
        r#"SELECT site_id, COUNT(*)::bigint AS cnt
           FROM waf_fp_remediation
           WHERE created_at >= $1
           GROUP BY site_id
           ORDER BY cnt DESC
           LIMIT 20"#,
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;

    let by_site = site_rows
        .iter()
        .map(|r| FpSiteCount {
            site_id: r.try_get("site_id").unwrap_or_default(),
            count: r.try_get("cnt").unwrap_or(0),
        })
        .collect();

    Ok(FpAnalyticsSnapshot {
        confirmed_fp,
        top_rules,
        by_site,
    })
}
