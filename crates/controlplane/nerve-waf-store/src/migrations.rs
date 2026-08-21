use sqlx::PgPool;

use crate::StoreError;

/// Open the pool only. **Do not** run DDL here or in any Rust code.
/// Schema changes: edit `crates/nerve-waf-store/migrations/*.sql`, then run
/// `scripts/apply-migrations.sh` (or psql) manually; independent of the app process.
pub async fn connect_pool(database_url: &str) -> Result<PgPool, StoreError> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .map_err(|e| StoreError::Db(format!("connect failed: {e}")))
}
