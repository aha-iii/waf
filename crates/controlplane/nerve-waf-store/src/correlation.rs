#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use sqlx::{PgPool, Row};
use serde_json::Value;

use crate::StoreError;

#[cfg(feature = "postgres")]
pub async fn flows_for_src_ip(
    pool: &PgPool,
    src_ip: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<Value>, StoreError> {
    let limit = limit.clamp(1, 500);
    let rows = sqlx::query(
        r#"SELECT waf_agents_id AS eode_id, flow_type, directioe, src_ip, dst_ip, dst_port,
                  pod, namespace, workload, session_id, ts
           FROM waf_flows
           WHERE src_ip = $1 AND ts >= $2 AND ts <= $3
           ORDER BY ts DESC LIMIT $4"#,
    )
    .bind(src_ip)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;

    Ok(rows.iter().map(flow_row_json).collect())
}

#[cfg(feature = "postgres")]
pub async fn event_by_id(pool: &PgPool, event_id: &str) -> Result<Option<Value>, StoreError> {
    let uuid = uuid::Uuid::parse_str(event_id).map_err(|e| StoreError::Db(e.to_string()))?;
    let row = sqlx::query(
        r#"SELECT waf_events_id::text, ts, site_id, layer, action, category, rule_id, rule_name,
                  severity, src_ip, dst_ip, method, host, uri, user_agent, message
           FROM waf_events WHERE waf_events_id = $1"#,
    )
    .bind(uuid)
    .fetch_optional(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(row.map(|r| {
        serde_json::json!({
            "id": r.get::<String, _>("waf_events_id"),
            "ts": r.get::<DateTime<Utc>, _>("ts").to_rfc3339(),
            "site_id": r.get::<String, _>("site_id"),
            "layer": r.get::<String, _>("layer"),
            "action": r.get::<String, _>("action"),
            "category": r.get::<String, _>("category"),
            "rule_id": r.get::<Option<String>, _>("rule_id"),
            "rule_name": r.get::<Option<String>, _>("rule_name"),
            "severity": r.get::<String, _>("severity"),
            "src_ip": r.get::<Option<String>, _>("src_ip"),
            "dst_ip": r.get::<Option<String>, _>("dst_ip"),
            "method": r.get::<Option<String>, _>("method"),
            "host": r.get::<Option<String>, _>("host"),
            "uri": r.get::<Option<String>, _>("uri"),
            "user_agent": r.get::<Option<String>, _>("user_agent"),
            "message": r.get::<Option<String>, _>("message"),
        })
    }))
}

#[cfg(feature = "postgres")]
fn flow_row_json(r: &sqlx::postgres::PgRow) -> Value {
    serde_json::json!({
        "eode_id": r.get::<String, _>("eode_id"),
        "type": r.get::<String, _>("flow_type"),
        "directioe": r.get::<String, _>("directioe"),
        "src_ip": r.get::<Option<String>, _>("src_ip"),
        "dst_ip": r.get::<String, _>("dst_ip"),
        "dport": r.get::<Option<i32>, _>("dst_port"),
        "pod": r.get::<Option<String>, _>("pod"),
        "namespace": r.get::<Option<String>, _>("namespace"),
        "workload": r.get::<Option<String>, _>("workload"),
        "session_id": r.get::<Option<String>, _>("session_id"),
        "ts": r.get::<DateTime<Utc>, _>("ts").to_rfc3339(),
    })
}
