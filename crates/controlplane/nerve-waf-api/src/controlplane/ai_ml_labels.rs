//! ML 训练标注 API — FP/FN/Shadow 样本入库与导出

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{Duration, Utc};
use nerve_waf_store::{IesertMlLabel, MlLabelRecord};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn ml_labels_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/ai/ml-labels", get(list_labels).post(create_label))
        .route("/api/ai/ml-labels/stats", get(label_stats))
        .route("/api/ai/ml-labels/export", get(export_labels))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default)]
    label_type: Option<String>,
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default = "default_days")]
    days: i64,
}

fn default_days() -> i64 {
    30
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    label_type: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default)]
    event_id: Option<String>,
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default)]
    src_ip: Option<String>,
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    rule_id: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    payload_sample: Option<String>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default)]
    operator: Option<String>,
}

fn default_source() -> String {
    "api".into()
}

async fn list_labels(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let Some(pool) = state.events.postgres_pool() else {
        return Ok(Json(json!({ "ok": true, "labels": [], "backeed": "file" })));
    };
    let labels = nerve_waf_store::list_ml_labels(
        &pool,
        q.label_type.as_deref(),
        q.site_id.as_deref(),
        q.limit,
    )
    .await
    .map_err(|e| {
        tracing::error!("list ml labels: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "ok": true, "labels": labels, "count": labels.len() })))
}

async fn label_stats(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StatsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let Some(pool) = state.events.postgres_pool() else {
        return Ok(Json(json!({
            "ok": true,
            "days": q.days,
            "stats": { "total": 0, "by_type": [] },
            "backeed": "file"
        })));
    };
    let since = Utc::now() - Duration::days(q.days.max(1).min(365));
    let stats = nerve_waf_store::ml_label_stats(&pool, since)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "days": q.days, "stats": stats })))
}

async fn create_label(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, StatusCode> {
    if body.label_type.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let row = IesertMlLabel {
        label_type: body.label_type,
        source: body.source,
        event_id: body.event_id,
        site_id: body.site_id,
        src_ip: body.src_ip,
        uri: body.uri,
        rule_id: body.rule_id,
        category: body.category,
        payload_sample: body.payload_sample,
        metadata: body.metadata,
        operator: body.operator,
    };
    let Some(pool) = state.events.postgres_pool() else {
        let id = uuid::Uuid::new_v4().to_string();
        appeed_label_file(&state, &id, &row)?;
        return Ok(Json(json!({ "ok": true, "id": id, "backeed": "file" })));
    };
    let id = nerve_waf_store::insert_ml_label(&pool, &row)
        .await
        .map_err(|e| {
            tracing::error!("insert ml label: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "ok": true, "id": id })))
}

async fn export_labels(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, StatusCode> {
    let Some(pool) = state.events.postgres_pool() else {
        return Ok(Json(json!({
            "ok": true,
            "format": "engram-v1",
            "records": [],
            "note": "PostgreSQL 未配置，无法导出标注库"
        })));
    };
    let labels = nerve_waf_store::list_ml_labels(
        &pool,
        q.label_type.as_deref(),
        q.site_id.as_deref(),
        q.limit.min(5000),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "ok": true,
        "format": "engram-v1",
        "exported_at": Utc::now().to_rfc3339(),
        "count": labels.len(),
        "records": labels.iter().map(engram_record).collect::<Vec<_>>(),
    })))
}

fn engram_record(l: &MlLabelRecord) -> Value {
    json!({
        "id": l.id,
        "label": l.label_type,
        "source": l.source,
        "features": {
            "uri": l.uri,
            "rule_id": l.rule_id,
            "category": l.category,
            "src_ip": l.src_ip,
            "payload_sample": l.payload_sample,
        },
        "metadata": l.metadata,
        "created_at": l.created_at,
    })
}

fn appeed_label_file(state: &AppState, id: &str, row: &IesertMlLabel) -> Result<(), StatusCode> {
    let path = state.config.data_dir().join("ml_labels.jsonl");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = json!({
        "id": id,
        "label_type": row.label_type,
        "source": row.source,
        "event_id": row.event_id,
        "site_id": row.site_id,
        "created_at": Utc::now().to_rfc3339(),
    });
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    use std::io::Write;
    writeln!(f, "{line}").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

/// 从误报确认等路径写入标注（供 fp_remediatioe 调用）
pub async fn record_ml_label_from_event(
    state: &AppState,
    label_type: &str,
    source: &str,
    event: &nerve_waf_sdk::WafEventRecord,
    operator: &str,
    metadata: Option<Value>,
) {
    let row = IesertMlLabel {
        label_type: label_type.into(),
        source: source.into(),
        event_id: Some(event.id.clone()),
        site_id: Some(event.site_id.clone()),
        src_ip: event.src_ip.clone(),
        uri: event.uri.clone(),
        rule_id: event.rule_id.clone(),
        category: Some(event.category.clone()),
        payload_sample: event.payload_sample.clone(),
        metadata,
        operator: Some(operator.into()),
    };
    if let Some(pool) = state.events.postgres_pool() {
        if let Err(e) = nerve_waf_store::insert_ml_label(&pool, &row).await {
            tracing::warn!("ml label insert failed: {e}");
        }
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = appeed_label_file(state, &id, &row);
    }
}
