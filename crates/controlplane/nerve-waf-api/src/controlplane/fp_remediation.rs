//! 误报治理 — 上报误报（反馈/审计）；可选显式 actions 做加白 / 关规则。

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use nerve_waf_sdk::{ModuleConfig, WafEventRecord};
use nerve_waf_semantic::{apply_auto_tune, now_iso, SemanticFeedback, SemanticStatsRecorder};
use nerve_waf_whitelist::{WhitelistMatch, WhitelistRule};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::triage_routes::resolve_event;
use crate::AppState;

pub fn fp_remediation_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/events/{event_id}/confirm-false-positive",
            post(confirm_false_positive),
        )
        .route("/api/analytics/false-positives", get(false_positive_analytics))
}

#[derive(Debug, Deserialize)]
struct ConfirmFpBody {
    #[serde(default)]
    site_id: String,
    #[serde(default)]
    actions: Vec<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    operator: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FpAnalyticsQuery {
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default = "default_days")]
    days: i64,
}

fn default_days() -> i64 {
    7
}

async fn confirm_false_positive(
    State(state): State<Arc<AppState>>,
    Path(event_id): Path<String>,
    Json(body): Json<ConfirmFpBody>,
) -> Result<Json<Value>, StatusCode> {
    let event = resolve_event(&state, None, Some(event_id.clone())).await?;
    let site_id = if body.site_id.trim().is_empty() {
        event.site_id.clone()
    } else {
        body.site_id.trim().to_string()
    };

    let actions = body.actions;
    // 默认仅上报误报（记反馈 + 审计）；加白路径 / 禁用规则需显式传入对应 actions
    let mut applied: Vec<String> = Vec::new();

    if actions.iter().any(|a| a == "whitelist_uri") {
        if let Some(tag) = apply_whitelist_uri(&state, &event, &site_id, body.note.as_deref())? {
            applied.push(tag);
        }
    }

    if actions.iter().any(|a| a == "disable_rule") {
        if let Some(tag) = apply_disable_rule(&state, &event, &site_id).await? {
            applied.push(tag);
        }
    }

    if applied.is_empty() {
        applied.push("reported".into());
    }

    record_semantic_feedback(&state, &event, &site_id, &applied, body.note.as_deref()).await?;

    let audit_id = record_fp_audit(
        &state,
        &event,
        &site_id,
        &applied,
        body.note.as_deref(),
        body.operator.as_deref().unwrap_or("console"),
    )
    .await?;

    Ok(Json(json!({
        "ok": true,
        "event_id": event_id,
        "site_id": site_id,
        "applied": applied,
        "audit_id": audit_id,
    })))
}

#[allow(dead_code)]
fn suggest_actions(event: &WafEventRecord) -> Vec<String> {
    let mut out = vec!["whitelist_uri".to_string()];
    if event.rule_id.is_some() {
        out.push("disable_rule".to_string());
    }
    out
}

fn uri_path(uri: &str) -> String {
    let path = uri.split('?').next().unwrap_or(uri);
    if path.is_empty() {
        "/".into()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn apply_whitelist_uri(
    state: &AppState,
    event: &WafEventRecord,
    site_id: &str,
    note: Option<&str>,
) -> Result<Option<String>, StatusCode> {
    let uri = event.uri.as_deref().unwrap_or("");
    if uri.is_empty() {
        return Ok(None);
    }
    let path = uri_path(uri);
    let host = event.host.as_deref().unwrap_or("");

    let mut cfg = state.platform.whitelist_config();
    if !cfg.enabled {
        cfg.enabled = true;
    }

    let rule_id = format!("fp-{}", &event.id[..event.id.len().min(8)]);
    let already = cfg.rules.iter().any(|r| {
        r.enabled
            && r.match_on.uri_prefixes.iter().any(|p| p == &path)
            && (host.is_empty()
                || r.match_on.hosts.is_empty()
                || r.match_on.hosts.iter().any(|h| h == "*" || h == host))
    });
    if already {
        return Ok(Some(format!("whitelist_uri:{path} (already exists)")));
    }

    let mut match_on = WhitelistMatch {
        uri_prefixes: vec![path.clone()],
        ..Default::default()
    };
    if !host.is_empty() {
        match_on.hosts = vec![host.to_string()];
    }

    let comment = note
        .map(|n| format!("误报确认 site={site_id}: {n}"))
        .unwrap_or_else(|| format!("误报确认 site={site_id} event={}", event.id));

    cfg.rules.push(WhitelistRule {
        id: rule_id,
        enabled: true,
        comment,
        match_on,
    });

    state
        .platform
        .set_whitelist_config(cfg)
        .map_err(|e| {
            tracing::error!("set whitelist: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Some(format!("whitelist_uri:{path}")))
}

async fn apply_disable_rule(
    state: &AppState,
    event: &WafEventRecord,
    site_id: &str,
) -> Result<Option<String>, StatusCode> {
    let rule_id = match &event.rule_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => return Ok(None),
    };

    let module_key = resolve_module_key(event);
    let mut modules = state
        .config
        .load_modules_for(site_id)
        .await
        .map_err(|e| {
            tracing::error!("load modules site={site_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let entry = modules
        .modules
        .entry(module_key.clone())
        .or_insert_with(ModuleConfig::default);

    if entry.disabled_rules.iter().any(|r| r == &rule_id) {
        return Ok(Some(format!("disabled_rule:{rule_id} (already disabled)")));
    }
    entry.disabled_rules.push(rule_id.clone());

    state
        .config
        .save_modules_for(site_id, &modules, "fp-copilot")
        .await
        .map_err(|e| {
            tracing::error!("save modules site={site_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state.refresh_l7_from_store().await;

    Ok(Some(format!("disabled_rule:{rule_id}@{module_key}")))
}

fn resolve_module_key(event: &WafEventRecord) -> String {
    let cat = event.category.trim();
    if !cat.is_empty() && cat != "custom" && cat != "l3_policy" {
        return cat.to_string();
    }
    if event
        .rule_id
        .as_deref()
        .is_some_and(|id| id.chars().next().is_some_and(|c| c.is_ascii_digit()))
    {
        return "crs".into();
    }
    "custom".into()
}

async fn record_semantic_feedback(
    state: &AppState,
    event: &WafEventRecord,
    site_id: &str,
    applied: &[String],
    note: Option<&str>,
) -> Result<(), StatusCode> {
    let mut fb = SemanticFeedback {
        ts: now_iso(),
        site_id: site_id.to_string(),
        event_id: Some(event.id.clone()),
        verdict: "false_positive".into(),
        note: note.map(|s| s.to_string()),
        param_path: event.message.as_ref().and_then(|m| {
            m.split("param=")
                .nth(1)
                .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
                .filter(|s| !s.is_empty())
        }),
        uri: event.uri.clone(),
        applied_action: if applied.is_empty() {
            None
        } else {
            Some(applied.join(";"))
        },
    };

    let mut cfg = state.ai_settings.semantic_config_for(site_id).normalized();
    let auto_tune = cfg.auto_tune.clone();
    let history = state.semantic_stats.as_ref().feedback_for_site(site_id);
    let tune = apply_auto_tune(&mut cfg, &auto_tune, &history, &fb);
    if tune.config_changed {
        let cfg = cfg.normalized();
        state
            .ai_settings
            .set_semantic_for(site_id, cfg)
            .await
            .map_err(|e| {
                tracing::error!("auto-tune save: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        fb.applied_action = Some(format!(
            "{};auto_tune:{}",
            fb.applied_action.unwrap_or_default(),
            tune.actions.join(",")
        ));
    }

    state.semantic_stats.as_ref().add_feedback(fb);
    let _ = state.semantic_stats.flush_dirty().await;
    Ok(())
}

async fn record_fp_audit(
    state: &AppState,
    event: &WafEventRecord,
    site_id: &str,
    applied: &[String],
    note: Option<&str>,
    operator: &str,
) -> Result<String, StatusCode> {
    let audit_id = Uuid::new_v4().to_string();
    let actions_json = serde_json::to_value(applied).unwrap_or(json!([]));

    if let Some(pool) = state.events.postgres_pool() {
        if nerve_waf_store::insert_fp_audit(
            &pool,
            &audit_id,
            &event.id,
            site_id,
            event.src_ip.as_deref(),
            event.uri.as_deref(),
            event.rule_id.as_deref(),
            &actions_json,
            note,
            operator,
        )
        .await
        .is_ok()
        {
            return Ok(audit_id);
        }
    }

    let path = state.config.data_dir().join("fp_remediation_audit.jsonl");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = json!({
        "audit_id": audit_id,
        "event_id": event.id,
        "site_id": site_id,
        "src_ip": event.src_ip,
        "uri": event.uri,
        "rule_id": event.rule_id,
        "actions": applied,
        "note": note,
        "operator": operator,
        "created_at": Utc::now().to_rfc3339(),
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", line);
    }

    Ok(audit_id)
}

async fn false_positive_analytics(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FpAnalyticsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let since = Utc::now() - Duration::days(q.days.max(1).min(90));
    let site_filter = q.site_id.as_deref().filter(|s| !s.trim().is_empty());

    let mut confirmed_fp = 0i64;
    let mut top_rules: Vec<Value> = Vec::new();
    let mut by_site: Vec<Value> = Vec::new();

    if let Some(pool) = state.events.postgres_pool() {
        if let Ok(snap) = nerve_waf_store::fp_analytics(&pool, since, site_filter).await {
            confirmed_fp = snap.confirmed_fp;
            top_rules = snap
                .top_rules
                .iter()
                .map(|r| json!({ "rule_id": r.rule_id, "count": r.count }))
                .collect();
            by_site = snap
                .by_site
                .iter()
                .map(|s| json!({ "site_id": s.site_id, "count": s.count }))
                .collect();
        }
    }

    let stats = state
        .events
        .stats_since(since)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let blocked = stats.blocked as i64;
    let rate = if blocked > 0 {
        (confirmed_fp as f64 / blocked as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    Ok(Json(json!({
        "days": q.days,
        "site_id": site_filter,
        "total_blocks": blocked,
        "confirmed_fp": confirmed_fp,
        "false_positive_rate": (rate * 10.0).round() / 10.0,
        "top_rules": top_rules,
        "by_site": by_site,
    })))
}
