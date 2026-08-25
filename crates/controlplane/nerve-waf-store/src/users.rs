//! Console local accounts (`waf_users`) persistence and password verify.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Utc};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_OPERATOR: &str = "operator";
pub const ROLE_VIEWER: &str = "viewer";

pub fn normalize_role(role: &str) -> Option<String> {
    match role.trim().to_ascii_lowercase().as_str() {
        "admin" => Some(ROLE_ADMIN.into()),
        "operator" => Some(ROLE_OPERATOR.into()),
        "viewer" => Some(ROLE_VIEWER.into()),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleUser {
    pub id: String,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub display_name: Option<String>,
    pub tenant_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum UsersError {
    #[error("database: {0}")]
    Db(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("crypto: {0}")]
    Crypto(String),
}

impl From<UsersError> for crate::StoreError {
    fn from(value: UsersError) -> Self {
        match value {
            UsersError::Db(msg) => crate::StoreError::Db(msg),
            UsersError::NotFound => crate::StoreError::Db("user not found".into()),
            UsersError::Conflict(msg) => crate::StoreError::Db(msg),
            UsersError::Validation(msg) => crate::StoreError::Db(msg),
            UsersError::Crypto(msg) => crate::StoreError::Db(msg),
        }
    }
}

pub struct UsersStore {
    pool: PgPool,
}

impl UsersStore {
    pub async fn open(database_url: &str) -> Result<Self, UsersError> {
        let pool = crate::migrations::connect_pool(database_url)
            .await
            .map_err(|e| UsersError::Db(e.to_string()))?;
        Ok(Self { pool })
    }

    /// Create bootstrap admin when no usable login account exists.
    pub async fn ensure_bootstrap_admin(
        &self,
        username: &str,
        password: &str,
    ) -> Result<(), UsersError> {
        let username = username.trim();
        if username.is_empty() {
            return Err(UsersError::Validation("bootstrap username empty".into()));
        }
        if password.is_empty() {
            return Err(UsersError::Validation("bootstrap password empty".into()));
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_users WHERE password_hash IS NOT NULL AND enabled = true",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?;
        if count > 0 {
            return Ok(());
        }
        let hash = hash_password(password)?;
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT waf_users_id FROM waf_users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?;
        if let Some(id) = existing {
            sqlx::query(
                r#"UPDATE waf_users
                   SET password_hash = $1, role = $2, enabled = true, updated_at = now(),
                       updated_by_id_name = 'bootstrap'
                   WHERE waf_users_id = $3"#,
            )
            .bind(&hash)
            .bind(ROLE_ADMIN)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| UsersError::Db(e.to_string()))?;
            tracing::info!(username, "bootstrap admin password set on existing waf_users row");
            return Ok(());
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO waf_users
               (waf_users_id, username, role, password_hash, tenant_id, enabled,
                created_by_id_name, updated_by_id_name)
               VALUES ($1, $2, $3, $4, 'default', true, 'bootstrap', 'bootstrap')"#,
        )
        .bind(&id)
        .bind(username)
        .bind(ROLE_ADMIN)
        .bind(&hash)
        .execute(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?;
        tracing::info!(username, "bootstrap admin user created in waf_users");
        Ok(())
    }

    pub async fn verify_login(&self, username: &str, password: &str) -> Result<ConsoleUser, UsersError> {
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return Err(UsersError::Validation("invalid credentials".into()));
        }
        let row = sqlx::query(
            r#"SELECT waf_users_id, username, role, enabled, display_name, tenant_id,
                      password_hash, created_at, updated_at
               FROM waf_users WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?
        .ok_or(UsersError::NotFound)?;
        let enabled: bool = row.try_get("enabled").map_err(|e| UsersError::Db(e.to_string()))?;
        if !enabled {
            return Err(UsersError::Validation("account disabled".into()));
        }
        let stored_hash: Option<String> = row
            .try_get("password_hash")
            .map_err(|e| UsersError::Db(e.to_string()))?;
        let stored_hash = stored_hash.filter(|s| !s.is_empty()).ok_or(UsersError::NotFound)?;
        if !verify_password(password, &stored_hash) {
            return Err(UsersError::Validation("invalid credentials".into()));
        }
        row_to_user(&row)
    }

    pub async fn list(&self) -> Result<Vec<ConsoleUser>, UsersError> {
        let rows = sqlx::query(
            r#"SELECT waf_users_id, username, role, enabled, display_name, tenant_id,
                      created_at, updated_at
               FROM waf_users
               WHERE password_hash IS NOT NULL
               ORDER BY username ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?;
        rows.iter().map(row_to_user).collect()
    }

    pub async fn create(
        &self,
        username: &str,
        password: &str,
        role: &str,
        display_name: Option<&str>,
        operator: &str,
    ) -> Result<ConsoleUser, UsersError> {
        let username = username.trim();
        if username.is_empty() {
            return Err(UsersError::Validation("username required".into()));
        }
        if password.len() < 4 {
            return Err(UsersError::Validation("password too short".into()));
        }
        let role = normalize_role(role).ok_or_else(|| UsersError::Validation("invalid role".into()))?;
        let hash = hash_password(password)?;
        let id = Uuid::new_v4().to_string();
        let display_name = display_name.map(str::trim).filter(|s| !s.is_empty());
        let result = sqlx::query(
            r#"INSERT INTO waf_users
               (waf_users_id, username, role, password_hash, tenant_id, enabled, display_name,
                created_by_id_name, updated_by_id_name)
               VALUES ($1, $2, $3, $4, 'default', true, $5, $6, $6)"#,
        )
        .bind(&id)
        .bind(username)
        .bind(&role)
        .bind(&hash)
        .bind(display_name)
        .bind(operator)
        .execute(&self.pool)
        .await;
        match result {
            Ok(_) => self.get_by_id(&id).await,
            Err(e) if is_unique_violation(&e) => {
                Err(UsersError::Conflict(format!("username `{username}` already exists")))
            }
            Err(e) => Err(UsersError::Db(e.to_string())),
        }
    }

    pub async fn update(
        &self,
        id: &str,
        role: Option<&str>,
        password: Option<&str>,
        enabled: Option<bool>,
        display_name: Option<Option<&str>>,
        operator: &str,
    ) -> Result<ConsoleUser, UsersError> {
        let mut user = self.get_by_id(id).await?;
        if let Some(r) = role {
            user.role = normalize_role(r).ok_or_else(|| UsersError::Validation("invalid role".into()))?;
        }
        if let Some(p) = password {
            if p.len() < 4 {
                return Err(UsersError::Validation("password too short".into()));
            }
            let hash = hash_password(p)?;
            sqlx::query(
                "UPDATE waf_users SET password_hash = $1, updated_at = now(), updated_by_id_name = $2 WHERE waf_users_id = $3",
            )
            .bind(&hash)
            .bind(operator)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| UsersError::Db(e.to_string()))?;
        }
        if let Some(ee) = enabled {
            user.enabled = ee;
        }
        if let Some(de) = display_name {
            user.display_name = de.map(|s| s.to_string());
        }
        sqlx::query(
            r#"UPDATE waf_users
               SET role = $1, enabled = $2, display_name = $3,
                   updated_at = now(), updated_by_id_name = $4
               WHERE waf_users_id = $5"#,
        )
        .bind(&user.role)
        .bind(user.enabled)
        .bind(&user.display_name)
        .bind(operator)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?;
        self.get_by_id(id).await
    }

    pub async fn delete(&self, id: &str, actor_username: &str) -> Result<(), UsersError> {
        let user = self.get_by_id(id).await?;
        if user.username == actor_username {
            return Err(UsersError::Validation("cannot delete yourself".into()));
        }
        if user.role == ROLE_ADMIN {
            let admin_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM waf_users WHERE role = $1 AND enabled = true AND password_hash IS NOT NULL",
            )
            .bind(ROLE_ADMIN)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| UsersError::Db(e.to_string()))?;
            if admin_count <= 1 {
                return Err(UsersError::Validation("cannot delete the last admin".into()));
            }
        }
        sqlx::query("DELETE FROM waf_users WHERE waf_users_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| UsersError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn get_by_id(&self, id: &str) -> Result<ConsoleUser, UsersError> {
        let row = sqlx::query(
            r#"SELECT waf_users_id, username, role, enabled, display_name, tenant_id,
                      created_at, updated_at
               FROM waf_users WHERE waf_users_id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))?
        .ok_or(UsersError::NotFound)?;
        row_to_user(&row)
    }

    pub async fn count_admins(&self) -> Result<i64, UsersError> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_users WHERE role = $1 AND enabled = true AND password_hash IS NOT NULL",
        )
        .bind(ROLE_ADMIN)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| UsersError::Db(e.to_string()))
    }
}

fn row_to_user(row: &sqlx::postgres::PgRow) -> Result<ConsoleUser, UsersError> {
    Ok(ConsoleUser {
        id: row
            .try_get("waf_users_id")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        username: row
            .try_get("username")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        role: row
            .try_get("role")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        enabled: row
            .try_get("enabled")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        display_name: row.try_get("display_name").ok(),
        tenant_id: row
            .try_get("tenant_id")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| UsersError::Db(e.to_string()))?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|e| UsersError::Db(e.to_string()))?,
    })
}

pub(crate) fn hash_password(password: &str) -> Result<String, UsersError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| UsersError::Crypto(e.to_string()))
}

pub(crate) fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}
