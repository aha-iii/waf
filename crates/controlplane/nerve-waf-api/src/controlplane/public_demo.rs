//! 官网预约演示公开接口（免登录）。

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use nerve_waf_store::{DemoLead, NewDemoLead};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn public_demo_router() -> Router<Arc<AppState>> {
    Router::new().route("/api/public/demo", post(create_demo_lead))
}

#[derive(Debug, Deserialize)]
pub struct DemoLeadBody {
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
}

#[derive(Serialize)]
struct DemoLeadResp {
    ok: bool,
    id: String,
}

async fn create_demo_lead(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<DemoLeadBody>,
) -> Result<Json<DemoLeadResp>, (StatusCode, Json<Value>)> {
    let store = state.demo_leads_store.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "demo leads store unavailable" })),
        )
    })?;

    let client_ip = client_ip_from_headers(&headers);
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let lead = store
        .insert(NewDemoLead {
            name: body.name,
            email: body.email,
            company: body.company,
            phone: body.phone,
            note: body.note,
            product: body.product.unwrap_or_else(|| "waf".into()),
            source_path: body.source_path,
            client_ip,
            user_agent,
        })
        .await
        .map_err(|e| match e {
            nerve_waf_store::DemoLeadsError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": msg })))
            }
            nerve_waf_store::DemoLeadsError::Db(msg) => {
                tracing::error!(error = %msg, "demo_leads insert failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "persist failed" })),
                )
            }
        })?;

    let notified = notify_wecom(&lead).await;
    if let Err(e) = store.mark_wecom_notified(&lead.id, notified).await {
        tracing::warn!(error = %e, id = %lead.id, "mark wecom_notified failed");
    }

    Ok(Json(DemoLeadResp {
        ok: true,
        id: lead.id,
    }))
}

fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
}

async fn notify_wecom(lead: &DemoLead) -> bool {
    let url = match std::env::var("DEMO_WECOM_WEBHOOK_URL") {
        Ok(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => {
            tracing::warn!("DEMO_WECOM_WEBHOOK_URL unset; skip wecom notify");
            return false;
        }
    };

    let content = format!(
        "**官网预约演示**\n> 姓名：{name}\n> 邮箱：{email}\n> 公司：{company}\n> 电话：{phone}\n> 产品：{product}\n> 来源：{source}\n> 备注：{note}\n> 时间：{ts}\n> ID：`{id}`",
        name = lead.name,
        email = lead.email,
        company = lead.company.as_deref().unwrap_or("-"),
        phone = lead.phone.as_deref().unwrap_or("-"),
        product = lead.product,
        source = lead.source_path.as_deref().unwrap_or("-"),
        note = lead.note.as_deref().unwrap_or("-"),
        ts = lead.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
        id = lead.id,
    );

    let body = json!({
        "msgtype": "markdown",
        "markdown": { "content": content }
    });

    match reqwest::Client::new()
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => true,
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!(%status, body = %text, "wecom webhook non-success");
            false
        }
        Err(e) => {
            tracing::warn!(error = %e, "wecom webhook request failed");
            false
        }
    }
}
