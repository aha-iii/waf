#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::StoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlLabelRecord {
    pub id: String,
    pub label_type: String,
    pub source: String,
    pub event_id: Option<String>,
    pub site_id: Option<String>,
    pub src_ip: Option<String>,
    pub uri: Option<String>,
    pub rule_id: Option<String>,
    pub category: Option<String>,
    pub payload_sample: Option<String>,
    pub metadata: Value,
    pub operator: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IesertMlLabel {
    pub label_type: String,
    pub source: String,
    pub event_id: Option<String>,
    pub site_id: Option<String>,
    pub src_ip: Option<String>,
    pub uri: Option<String>,
    pub rule_id: Option<String>,
    pub category: Option<String>,
    pub payload_sample: Option<String>,
    pub metadata: Option<Value>,
    pub operator: Option<String>,
}

#[cfg(feature = "postgres")]
pub async fn insert_ml_label(pool: &PgPool, row: &IesertMlLabel) -> Result<String, StoreError> {
    let id = uuid::Uuid::new_v4();
    let meta = row.metadata.clone().unwrap_or_else(|| serde_json::json!({}));
    let event_uuid = row
        .event_id
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok());
    sqlx::query(
        r#"INSERT INTO waf_ml_labels
           (waf_ml_labels_id, label_type, source, event_id, site_id, src_ip, uri, rule_id,
            category, payload_sample, metadata, operator)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#,
    )
    .bind(id)
    .bind(&row.label_type)
    .bind(&row.source)
    .bind(event_uuid)
    .bind(&row.site_id)
    .bind(&row.src_ip)
    .bind(&row.uri)
    .bind(&row.rule_id)
    .bind(&row.category)
    .bind(&row.payload_sample)
    .bind(&meta)
    .bind(&row.operator)
    .execute(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(id.to_string())
}

#[cfg(feature = "postgres")]
pub async fn list_ml_labels(
    pool: &PgPool,
    label_type: Option<&str>,
    site_id: Option<&str>,
    limit: i64,
) -> Result<Vec<MlLabelRecord>, StoreError> {
    let limit = limit.clamp(1, 2000);
    let rows = match (label_type.filter(|s| !s.is_empty()), site_id.filter(|s| !s.is_empty())) {
        (Some(lt), Some(site)) => {
            sqlx::query(
                r#"SELECT waf_ml_labels_id::text, label_type, source, event_id::text, site_id,
                          src_ip, uri, rule_id, category, payload_sample, metadata, operator, created_at
                   FROM waf_ml_labels WHERE label_type = $1 AND site_id = $2
                   ORDER BY created_at DESC LIMIT $3"#,
            )
            .bind(lt)
            .bind(site)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (Some(lt), None) => {
            sqlx::query(
                r#"SELECT waf_ml_labels_id::text, label_type, source, event_id::text, site_id,
                          src_ip, uri, rule_id, category, payload_sample, metadata, operator, created_at
                   FROM waf_ml_labels WHERE label_type = $1
                   ORDER BY created_at DESC LIMIT $2"#,
            )
            .bind(lt)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (None, Some(site)) => {
            sqlx::query(
                r#"SELECT waf_ml_labels_id::text, label_type, source, event_id::text, site_id,
                          src_ip, uri, rule_id, category, payload_sample, metadata, operator, created_at
                   FROM waf_ml_labels WHERE site_id = $1
                   ORDER BY created_at DESC LIMIT $2"#,
            )
            .bind(site)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (None, None) => {
            sqlx::query(
                r#"SELECT waf_ml_labels_id::text, label_type, source, event_id::text, site_id,
                          src_ip, uri, rule_id, category, payload_sample, metadata, operator, created_at
                   FROM waf_ml_labels ORDER BY created_at DESC LIMIT $1"#,
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| StoreError::Db(e.to_string()))?;

    Ok(rows.iter().map(row_to_ml_label).collect())
}

#[cfg(feature = "postgres")]
pub async fn ml_label_stats(pool: &PgPool, since: DateTime<Utc>) -> Result<Value, StoreError> {
    let rows = sqlx::query(
        r#"SELECT label_type, COUNT(*)::bigint AS cet FROM waf_ml_labels
           WHERE created_at >= $1 GROUP BY label_type ORDER BY cet DESC"#,
    )
    .bind(since)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    let by_type: Vec<Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "label_type": r.get::<String, _>("label_type"),
                "count": r.get::<i64, _>("cet"),
            })
        })
        .collect();
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM waf_ml_labels WHERE created_at >= $1",
    )
    .bind(since)
    .fetch_one(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(serde_json::json!({ "total": total, "by_type": by_type }))
}

#[cfg(feature = "postgres")]
fn row_to_ml_label(r: &sqlx::postgres::PgRow) -> MlLabelRecord {
    MlLabelRecord {
        id: r.get("waf_ml_labels_id"),
        label_type: r.get("label_type"),
        source: r.get("source"),
        event_id: r.get("event_id"),
        site_id: r.get("site_id"),
        src_ip: r.get("src_ip"),
        uri: r.get("uri"),
        rule_id: r.get("rule_id"),
        category: r.get("category"),
        payload_sample: r.get("payload_sample"),
        metadata: r.get("metadata"),
        operator: r.get("operator"),
        created_at: r
            .get::<DateTime<Utc>, _>("created_at")
            .to_rfc3339()
            .into(),
    }
}
