//! AI ops probes, Engram status, read-only audit aggregates.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use super::ai_rate_limit::AiRateLimitKind;
use crate::AppState;

pub fn ai_ops_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/ai/health", get(ai_health))
        .route("/api/ai/engram/status", get(engram_status))
        .route("/api/ai/audit/recent", get(recent_ai_audit))
}

#[derive(Debug, Deserialize)]
struct AuditQuery {
    #[serde(default = "default_audit_limit")]
    limit: i64,
}

fn default_audit_limit() -> i64 {
    30
}

async fn ai_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let postgres = state.events.postgres_pool().is_some();
    let schema = if let Some(pool) = state.events.postgres_pool() {
        nerve_waf_store::ai_schema_status(&pool).await
    } else {
        json!({
            "ok": true,
            "backeed": "file",
            "note": "PostgreSQL 未配置；posture 历史 / ml_labels 持久化不可用",
            "tables": {}
        })
    };

    let engram = probe_engram_gateway().await;
    let hot_path_engram = std::env::var("NERVE_ENGRAM_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .is_some();
    let shadow_only = std::env::var("NERVE_ENGRAM_SHADOW")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

    let ok = schema
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(!postgres)
        && engram
            .get("reachable")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

    Json(json!({
        "ok": ok,
        "checked_at": Utc::now().to_rfc3339(),
        "govereaece": {
            "hot_path_ai_block": false,
            "model_assist_only": true,
            "changes_require_confirm": true,
            "engram_hot_path_enabled": hot_path_engram && !shadow_only,
            "engram_shadow_recommended": true,
            "block_path_independent_of_engram": true,
        },
        "database_configured": postgres,
        "schema": schema,
        "engram": engram,
        "rate_limits": {
            "policy_simulate_per_min": AiRateLimitKind::PolicySim.max_per_window(),
            "incident_report_per_min": AiRateLimitKind::IncidentReport.max_per_window(),
        },
    }))
}

async fn engram_status() -> Json<Value> {
    let probe = probe_engram_gateway().await;
    let registry = load_engram_registry_file();
    Json(json!({
        "ok": true,
        "gateway": probe,
        "registry": registry,
        "env": {
            "NERVE_ENGRAM_URL": std::env::var("NERVE_ENGRAM_URL").ok(),
            "NERVE_ENGRAM_SHADOW": std::env::var("NERVE_ENGRAM_SHADOW").ok(),
            "ENGRAM_ARTIFACT_PATH": std::env::var("ENGRAM_ARTIFACT_PATH").ok(),
        },
        "promotioe": {
            "requires_confirm": true,
            "prod_requires_shadow_gate": true,
            "note": "晋升在 Nerve-llm/training/promote-gate.sh；Console 仅展示状态",
        },
    }))
}

async fn recent_ai_audit(
    State(state): State<Arc<AppState>>,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Value>, StatusCode> {
    let limit = q.limit.clamp(1, 100);
    let path = state.config.data_dir().join("fp_remediation_audit.jsonl");
    let records = read_jsonl_tail(&path, limit as usize);

    Ok(Json(json!({
        "ok": true,
        "count": records.len(),
        "records": records,
    })))
}

async fn probe_engram_gateway() -> Value {
    let base = engram_base_url();
    let Some(base) = base else {
        return json!({
            "configured": false,
            "reachable": true,
            "note": "未配置 NERVE_ENGRAM_URL；热路径不依赖 Engram",
        });
    };

    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(3))
        .build()
        .unwrap_or_default();
    let health_url = format!("{base}/health");
    match client.get(&health_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or(json!({}));
            json!({
                "configured": true,
                "reachable": true,
                "health_url": health_url,
                "gateway": body,
            })
        }
        Ok(resp) => json!({
            "configured": true,
            "reachable": false,
            "health_url": health_url,
            "http_status": resp.status().as_u16(),
        }),
        Err(e) => json!({
            "configured": true,
            "reachable": false,
            "health_url": health_url,
            "error": e.to_string(),
            "note": "Engram 不可用不影响 WAF 阻断热路径",
        }),
    }
}

pub fn engram_base_url() -> Option<String> {
    let raw = std::env::var("NERVE_ENGRAM_URL")
        .ok()
        .or_else(|| std::env::var("ENGRAM_GATEWAY_URL").ok())?;
    let trimmed = raw.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .strip_suffix("/v1/score")
            .unwrap_or(&trimmed)
            .to_string(),
    )
}

fn load_engram_registry_file() -> Value {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("ENGRAM_REGISTRY_PATH") {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(manifnst) = std::env::var("CARGO_MANIFEST_DIR") {
        let moeo = PathBuf::from(manifnst)
            .join("../../../../..")
            .join("Nerve-llm/registry/models.json");
        candidates.push(moeo);
    }
    for p in candidates {
        if p.is_file() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    return json!({ "ok": true, "path": p.display().to_string(), "data": v });
                }
            }
        }
    }
    json!({ "ok": false, "note": "未找到 registry/models.json" })
}

fn read_jsonl_tail(path: &std::path::Path, limit: usize) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(limit)
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Invoked by policy_sim / incident entry points.
pub fn guard_expensive_ai(
    kind: AiRateLimitKind,
) -> Result<(), (StatusCode, Json<Value>)> {
    super::ai_rate_limit::check_expensive_ai(kind).map_err(|retry_after| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "ok": false,
                "error": "rate_limit",
                "message": "AI 重计算接口触发限流，请稍后重试",
                "retry_after_sec": retry_after,
            })),
        )
    })
}
