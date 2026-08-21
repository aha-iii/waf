//! Protected-app list (Console `/api/sites`) PostgreSQL persistence.

use serde_json::Value;
use sqlx::{PgPool, Row};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtectedSitesError {
    #[error("database: {0}")]
    Db(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<ProtectedSitesError> for crate::StoreError {
    fn from(value: ProtectedSitesError) -> Self {
        match value {
            ProtectedSitesError::Db(msg) => crate::StoreError::Db(msg),
            ProtectedSitesError::Json(e) => crate::StoreError::Json(e),
        }
    }
}

pub struct ProtectedSitesStore {
    pool: PgPool,
}

impl ProtectedSitesStore {
    pub async fn open(database_url: &str) -> Result<Self, ProtectedSitesError> {
        let pool = crate::migrations::connect_pool(database_url)
            .await
            .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn list(&self) -> Result<Vec<Value>, ProtectedSitesError> {
        let rows = sqlx::query(
            r#"SELECT body FROM waf_protected_sites ORDER BY sort_order ASC, site_id ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        rows.into_iter()
            .map(|row| {
                let body: Value = row
                    .try_get("body")
                    .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
                Ok(body)
            })
            .collect()
    }

    pub async fn replace_all(
        &self,
        sites: &[Value],
        operator: &str,
    ) -> Result<(), ProtectedSitesError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        sqlx::query("DELETE FROM waf_protected_sites")
            .execute(&mut *tx)
            .await
            .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        for (idx, site) in sites.iter().enumerate() {
            let site_id = site
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProtectedSitesError::Db("site id missing in body".into()))?;
            let name = site
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProtectedSitesError::Db(format!("site {site_id} name missing")))?;
            let hosts = site.get("hosts").cloned().unwrap_or_else(|| Value::Array(vec![]));
            let enabled = site
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            sqlx::query(
                r#"INSERT INTO waf_protected_sites
                   (site_id, name, hosts, enabled, body, sort_order, created_by_id_name, updated_by_id_name)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $7)"#,
            )
            .bind(site_id)
            .bind(name)
            .bind(hosts)
            .bind(enabled)
            .bind(site)
            .bind(i32::try_from(idx).unwrap_or(i32::MAX))
            .bind(operator)
            .execute(&mut *tx)
            .await
            .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ProtectedSitesError::Db(e.to_string()))?;
        Ok(())
    }
}
