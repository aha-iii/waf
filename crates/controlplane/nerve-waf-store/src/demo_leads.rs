//! Website demo-booking leads (`demo_leads`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoLead {
    pub id: String,
    pub name: String,
    pub email: String,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    pub product: String,
    pub source_path: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub wecom_notified: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewDemoLead {
    pub name: String,
    pub email: String,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    pub product: String,
    pub source_path: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Error)]
pub enum DemoLeadsError {
    #[error("database: {0}")]
    Db(String),
    #[error("validation: {0}")]
    Validation(String),
}

pub struct DemoLeadsStore {
    pool: PgPool,
}

impl DemoLeadsStore {
    pub async fn open(database_url: &str) -> Result<Self, DemoLeadsError> {
        let pool = crate::migrations::connect_pool(database_url)
            .await
            .map_err(|e| DemoLeadsError::Db(e.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn insert(&self, lead: NewDemoLead) -> Result<DemoLead, DemoLeadsError> {
        let name = lead.name.trim();
        let email = lead.email.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(DemoLeadsError::Validation("name required".into()));
        }
        if name.chars().count() > 80 {
            return Err(DemoLeadsError::Validation("name too long".into()));
        }
        if email.is_empty() || !email.contains('@') || email.len() > 160 {
            return Err(DemoLeadsError::Validation("email invalid".into()));
        }
        let company = trim_opt(&lead.company, 120);
        let phone = trim_opt(&lead.phone, 40);
        let note = trim_opt(&lead.note, 2000);
        let product = {
            let p = lead.product.trim();
            if p.is_empty() {
                "waf".into()
            } else if p.len() > 32 {
                return Err(DemoLeadsError::Validation("product too long".into()));
            } else {
                p.to_string()
            }
        };
        let source_path = trim_opt(&lead.source_path, 240);
        let client_ip = trim_opt(&lead.client_ip, 64);
        let user_agent = trim_opt(&lead.user_agent, 400);

        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"INSERT INTO demo_leads
               (id, name, email, company, phone, note, product, source_path, client_ip, user_agent, wecom_notified)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,false)
               RETURNING id, name, email, company, phone, note, product, source_path, client_ip, user_agent,
                         wecom_notified, created_at"#,
        )
        .bind(id)
        .bind(name)
        .bind(&email)
        .bind(&company)
        .bind(&phone)
        .bind(&note)
        .bind(&product)
        .bind(&source_path)
        .bind(&client_ip)
        .bind(&user_agent)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DemoLeadsError::Db(e.to_string()))?;

        Ok(map_row(&row))
    }

    pub async fn mark_wecom_notified(&self, id: &str, ok: bool) -> Result<(), DemoLeadsError> {
        let uid = Uuid::parse_str(id).map_err(|e| DemoLeadsError::Validation(e.to_string()))?;
        sqlx::query("UPDATE demo_leads SET wecom_notified = $1 WHERE id = $2")
            .bind(ok)
            .bind(uid)
            .execute(&self.pool)
            .await
            .map_err(|e| DemoLeadsError::Db(e.to_string()))?;
        Ok(())
    }
}

fn trim_opt(v: &Option<String>, max: usize) -> Option<String> {
    v.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else if t.chars().count() > max {
            Some(t.chars().take(max).collect())
        } else {
            Some(t.to_string())
        }
    })
}

fn map_row(row: &sqlx::postgres::PgRow) -> DemoLead {
    let id: Uuid = row.get("id");
    DemoLead {
        id: id.to_string(),
        name: row.get("name"),
        email: row.get("email"),
        company: row.get("company"),
        phone: row.get("phone"),
        note: row.get("note"),
        product: row.get("product"),
        source_path: row.get("source_path"),
        client_ip: row.get("client_ip"),
        user_agent: row.get("user_agent"),
        wecom_notified: row.get("wecom_notified"),
        created_at: row.get("created_at"),
    }
}
