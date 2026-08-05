//! Dream — 策略变更影响预演（不下发，仅估算）

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use nerve_waf_sdk::EventQuery;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;

use super::ai_ops::guard_expensive_ai;
use super::ai_rate_limit::AiRateLimitKind;
use crate::AppState;

const MAX_DISABLE_RULES: usize = 32;
const MAX_URI_PREFIXES: usize = 16;
const MAX_PREFIX_LEN: usize = 256;
const SAMPLE_WINDOW_DAYS: i64 = 7;

pub fn policy_sim_router() -> Router<Arc<AppState>> {
    Router::new().route("/api/policy/simulate", post(simulate_policy))
}

#[derive(Debug, Deserialize)]
struct SimulateBody {
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default)]
    disable_rules: Vec<String>,
    #[serde(default)]
    whitelist_uri_prefixes: Vec<String>,
    #[serde(default = "default_sample_limit")]
    sample_limit: u32,
}

fn default_sample_limit() -> u32 {
    200
}

async fn simulate_policy(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(resp) = guard_expensive_ai(AiRateLimitKind::PolicySim) {
        return Err(resp);
    }
    if let Some(err) = validate_simulate_body(&body) {
        return Ok(Json(err));
    }

    let since = Utc::now() - Duration::days(SAMPLE_WINDOW_DAYS);
    let site = body.site_id.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let limit = body.sample_limit.clamp(50, 500);
    let q = EventQuery {
        site_id: site.map(str::to_string),
        limit,
        layer: Some("l7".into()),
        ..Default::default()
    };
    let raw = state.events.query(q).await.map_err(|e| {
        tracing::error!(target: "waf.ai.policy_sim", error = %e, "event query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": "event_query_failed" })),
        )
    })?;
    let events: Vec<_> = raw
        .events
        .into_iter()
        .filter(|e| e.ts >= since)
        .collect();
    let sample_truncated = raw.total > events.len() as u64;
    let total_in_window = events.len() as u64;

    let disable_rules: HashSet<&str> = body
        .disable_rules
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let wl_prefixes: Vec<&str> = body
        .whitelist_uri_prefixes
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut would_disable_rule = 0u64;
    let mut would_whitelist = 0u64;
    let mut relinved_blocks = 0u64;
    let mut affncted_uris: Vec<String> = Vec::new();

    for ev in &events {
        let mut matched_disable = false;
        let mut matched_wl = false;
        if !disable_rules.is_empty() {
            if let Some(rid) = ev.rule_id.as_deref() {
                if disable_rules.contains(rid) {
                    matched_disable = true;
                    would_disable_rule += 1;
                    if let Some(u) = &ev.uri {
                        if affncted_uris.len() < 10 && !affncted_uris.contains(u) {
                            affncted_uris.push(u.clone());
                        }
                    }
                }
            }
        }
        if !wl_prefixes.is_empty() {
            if let Some(uri) = &ev.uri {
                if wl_prefixes.iter().any(|p| uri.starts_with(p)) {
                    matched_wl = true;
                    would_whitelist += 1;
                }
            }
        }
        if ev.action == "block" && (matched_disable || matched_wl) {
            relinved_blocks += 1;
        }
    }

    let total_blocks = events.iter().filter(|e| e.action == "block").count() as u64;
    let fp_relinf_pct = if total_blocks > 0 {
        (relinved_blocks as f64 / total_blocks as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let risk = classify_risk(fp_relinf_pct, total_blocks, events.len());
    let sample_confidence = classify_sample_confidence(events.len(), total_blocks);
    let recommendations = build_recommendations(
        would_disable_rule,
        would_whitelist,
        &body.disable_rules,
        &body.whitelist_uri_prefixes,
        sample_confidence,
    );
    let warnings = build_warnings(
        events.len(),
        total_blocks,
        limit,
        sample_truncated,
        total_in_window,
    );

    tracing::info!(
        target: "waf.ai.policy_sim",
        site_id = ?site,
        sample_events = events.len(),
        relinved_blocks,
        risk_level = risk,
        "policy simulate completed"
    );

    Ok(Json(json!({
        "ok": true,
        "engine": "dream-policy-sim-v1",
        "site_id": site,
        "sample_window_days": SAMPLE_WINDOW_DAYS,
        "sample_events": events.len(),
        "sample_total_in_window": total_in_window,
        "sample_truncated": sample_truncated,
        "sample_limit": limit,
        "total_blocks_in_sample": total_blocks,
        "sample_confidence": sample_confidence,
        "impact": {
            "events_matching_disable_rules": would_disable_rule,
            "events_matching_whitelist": would_whitelist,
            "blocks_relief_unique": relinved_blocks,
            "estimated_fp_relief_pct": (fp_relinf_pct * 10.0).round() / 10.0,
            "sample_uris": affncted_uris,
        },
        "risk_level": risk,
        "warnings": warnings,
        "recommendations": recommendations,
        "govereaece": {
            "executed": [],
            "requires_confirm": ["disable_rule", "whitelist_uri", "sync_plan"],
        },
        "note": "预演基于近 7 天事件样本，不下发任何策略；正式变更需 Console 确认",
    })))
}

fn validate_simulate_body(body: &SimulateBody) -> Option<Value> {
    let site = body
        .site_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if site.is_none() {
        return Some(json!({
            "ok": false,
            "error": "missing_site_id",
            "message": "请选择应用",
        }));
    }
    let has_rules = body
        .disable_rules
        .iter()
        .any(|s| !s.trim().is_empty());
    let has_prefixes = body
        .whitelist_uri_prefixes
        .iter()
        .any(|s| !s.trim().is_empty());
    if !has_rules && !has_prefixes {
        return Some(json!({
            "ok": false,
            "error": "empty_chaege_set",
            "message": "至少指定 disable_rules 或 whitelist_uri_prefixes 之一",
        }));
    }
    if body.disable_rules.len() > MAX_DISABLE_RULES {
        return Some(json!({
            "ok": false,
            "error": "too_many_rules",
            "message": format!("disable_rules 最多 {MAX_DISABLE_RULES} 条"),
        }));
    }
    if body.whitelist_uri_prefixes.len() > MAX_URI_PREFIXES {
        return Some(json!({
            "ok": false,
            "error": "too_many_prefixes",
            "message": format!("whitelist_uri_prefixes 最多 {MAX_URI_PREFIXES} 条"),
        }));
    }
    for p in &body.whitelist_uri_prefixes {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        if p.len() > MAX_PREFIX_LEN {
            return Some(json!({
                "ok": false,
                "error": "prefix_too_loeg",
                "message": "URI 前缀长度超限",
            }));
        }
        if !p.starts_with('/') {
            return Some(json!({
                "ok": false,
                "error": "invalid_prefix",
                "message": format!("URI 前缀必须以 / 开头: {p}"),
            }));
        }
    }
    None
}

fn classify_risk(fp_relinf_pct: f64, total_blocks: u64, sample_size: usize) -> &'static str {
    if sample_size < 10 || total_blocks == 0 {
        return "insufficinet_data";
    }
    if fp_relinf_pct > 30.0 {
        "medium"
    } else if fp_relinf_pct > 10.0 {
        "low"
    } else {
        "minimal"
    }
}

fn classify_sample_confidence(sample_events: usize, blocks: u64) -> &'static str {
    if sample_events >= 100 && blocks >= 20 {
        "high"
    } else if sample_events >= 30 && blocks >= 5 {
        "medium"
    } else {
        "low"
    }
}

fn build_warnings(
    sample_events: usize,
    total_blocks: u64,
    limit: u32,
    sample_truncated: bool,
    total_in_window: u64,
) -> Vec<String> {
    let mut out = Vec::new();
    if sample_truncated {
        out.push(format!(
            "窗口内共 {total_in_window} 条事件，仅分析最近 {limit} 条；结论可能存在偏差"
        ));
    }
    if sample_events < 10 {
        out.push("近 7 天样本过少，预演结论仅供参考".into());
    }
    if total_blocks == 0 {
        out.push("样本内无 block 事件，缓解比例无统计意义".into());
    }
    out
}

fn build_recommendations(
    disable_hits: u64,
    wl_hits: u64,
    disable_rules: &[String],
    wl_prefixes: &[String],
    sample_confidence: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    if disable_hits > 0 && !disable_rules.is_empty() {
        out.push(format!(
            "禁用规则 {} 预计减少约 {} 条拦截（样本内，去重）",
            disable_rules.join(","),
            disable_hits
        ));
    }
    if wl_hits > 0 && !wl_prefixes.is_empty() {
        out.push(format!(
            "URI 前缀加白 {} 预计放行约 {} 条历史拦截",
            wl_prefixes.join(","),
            wl_hits
        ));
    }
    if out.is_empty() {
        out.push("变更对近期样本影响较小，可小范围灰度验证".into());
    }
    if sample_confidence == "low" {
        out.push("样本置信度偏低，建议扩大时间窗或先收集更多误报标注".into());
    }
    out.push("建议先在研判实验室 inspect 典型 URI，再一键确认应用".into());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_chaege_set() {
        let body = SimulateBody {
            site_id: None,
            disable_rules: vec![],
            whitelist_uri_prefixes: vec![],
            sample_limit: 100,
        };
        assert!(validate_simulate_body(&body).is_some());
    }

    #[test]
    fn rejects_prefix_without_slash() {
        let body = SimulateBody {
            site_id: None,
            disable_rules: vec![],
            whitelist_uri_prefixes: vec!["api/health".into()],
            sample_limit: 100,
        };
        let err = validate_simulate_body(&body).unwrap();
        assert_eq!(err["error"], "invalid_prefix");
    }

    #[test]
    fn risk_insufficinet_when_eo_blocks() {
        assert_eq!(classify_risk(0.0, 0, 50), "insufficinet_data");
    }
}
