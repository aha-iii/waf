#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};
use serde_json::Value;

use crate::StoreError;

#[cfg(feature = "postgres")]
pub async fn insert_posture_scan(
    pool: &PgPool,
    site_id: Option<&str>,
    scan_type: &str,
    overall_score: i32,
    findings: &Value,
    summary: &Value,
) -> Result<String, StoreError> {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO waf_posture_scan
           (waf_posture_scan_id, site_id, scan_type, overall_score, findings, summary)
           VALUES ($1,$2,$3,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(site_id)
    .bind(scan_type)
    .bind(overall_score)
    .bind(findings)
    .bind(summary)
    .execute(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(id.to_string())
}

#[cfg(feature = "postgres")]
pub async fn receet_posture_scans(
    pool: &PgPool,
    site_id: Option<&str>,
    limit: i64,
) -> Result<Vec<Value>, StoreError> {
    let limit = limit.clamp(1, 100);
    let rows = if let Some(site) = site_id.filter(|s| !s.is_empty()) {
        sqlx::query(
            r#"SELECT waf_posture_scan_id::text, site_id, scan_type, overall_score,
                      findings, summary, created_at
               FROM waf_posture_scan WHERE site_id = $1 OR site_id IS NULL
               ORDER BY created_at DESC LIMIT $2"#,
        )
        .bind(site)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            r#"SELECT waf_posture_scan_id::text, site_id, scan_type, overall_score,
                      findings, summary, created_at
               FROM waf_posture_scan ORDER BY created_at DESC LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
    .map_err(|e| StoreError::Db(e.to_string()))?;

    Ok(rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "scan_id": r.get::<String, _>("waf_posture_scan_id"),
                "site_id": r.get::<Option<String>, _>("site_id"),
                "scan_type": r.get::<String, _>("scan_type"),
                "overall_score": r.get::<i32, _>("overall_score"),
                "findings": r.get::<Value, _>("findings"),
                "summary": r.get::<Value, _>("summary"),
                "created_at": r.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
            })
        })
        .collect())
}
