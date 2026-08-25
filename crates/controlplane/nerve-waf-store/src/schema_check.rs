#[cfg(feature = "postgres")]
use serde_json::{json, Value};
#[cfg(feature = "postgres")]
use sqlx::PgPool;

use crate::StoreError;

#[cfg(feature = "postgres")]
const AI_TABLES: &[&str] = &["waf_ml_labels", "waf_posture_scan", "waf_fp_remediation"];

/// Check whether AI-related migration tables are ready (035/036, etc.).
#[cfg(feature = "postgres")]
pub async fn ai_schema_status(pool: &PgPool) -> Value {
    let mut tables = serde_json::Map::new();
    let mut all_ok = true;
    for name in AI_TABLES {
        let exists = table_exists(pool, name).await.unwrap_or(false);
        if !exists {
            all_ok = false;
        }
        tables.insert(name.to_string(), json!({ "exists": exists }));
    }
    json!({
        "ok": all_ok,
        "backend": "postgres",
        "tables": tables,
        "migrate_hint": "bash scripts/apply-migrations.sh",
    })
}

#[cfg(feature = "postgres")]
async fn table_exists(pool: &PgPool, table: &str) -> Result<bool, StoreError> {
    let row: (bool,) = sqlx::query_as(
        r#"SELECT EXISTS (
             SELECT 1 FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name = $1
           )"#,
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .map_err(|e| StoreError::Db(e.to_string()))?;
    Ok(row.0)
}

/// AI migration dependency table names (035/036, etc.) for tests/docs alignment.
#[cfg(feature = "postgres")]
pub fn ai_required_tables() -> &'static [&'static str] {
    AI_TABLES
}
