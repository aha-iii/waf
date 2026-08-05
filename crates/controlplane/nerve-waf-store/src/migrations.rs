use sqlx::PgPool;

use crate::StoreError;

/// 仅建立连接池。**禁止**在此或任何 Rust 代码中执行 DDL。
/// 建表/改列：编辑 `crates/nerve-waf-store/migrations/*.sql`，用
/// `scripts/apply-migrations.sh`（或 psql）手动执行，与应用进程无关。
pub async fn connect_pool(database_url: &str) -> Result<PgPool, StoreError> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .map_err(|e| StoreError::Db(format!("connect failed: {e}")))
}
