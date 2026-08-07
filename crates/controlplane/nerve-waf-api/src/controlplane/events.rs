use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use nerve_waf_core::{build_waf_journey_hops, WafJourneyContext};
use nerve_waf_embed::EmbedInspectRequest;
use nerve_waf_challenge::{ChallengeStore, CHALLENGE_HEADER};
use nerve_waf_l7::InspectRequest;
use nerve_waf_sdk::{
    parse_traceparent, parse_tracestate, RequestTrace, HEADER_NERVE_JOURNEY_ID,
    HEADER_NERVE_PARENT_JOURNEY_ID, HEADER_NERVE_SPAN_ID, HEADER_NERVE_TRACE_ID,
    HEADER_TRACEPARENT, HEADER_TRACESTATE, HEADER_X_NERVE_TRACE_ID, HEADER_X_TRACE_ID,
    EventQuery, ProtectionModules, WafEventRecord,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use nerve_alert_sdk::{product::WAF, AlertPayload, DispatchRequest};
use crate::logging;
use crate::trace_middleware::NerveTrace;
use crate::triage_service::{should_suppress_alert, triage_alert_line, triage_many, triage_one, triage_to_json};
use crate::AppState;

#[derive(Deserialize)]
pub struct EventsQueryParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    pub category: Option<String>,
    pub action: Option<String>,
    pub src_ip: Option<String>,
    /// 按防护应用 ID 筛选
    pub site_id: Option<String>,
    /// l7 = WAF 攻击；l3 = 网络观测
    pub layer: Option<String>,
    /// 逗号分隔，排除的类型（攻击日志默认可在前端传 orphan_connect,anomaly）
    pub exclude_category: Option<String>,
    /// 为每条事件自动附加智能研判结果
    #[serde(default)]
    pub triage: bool,
}

fn default_limit() -> u32 {
    50
}

fn parse_exclude_categories(raw: Option<&str>) -> Option<Vec<String>> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }
    let cats: Vec<String> = s
        .split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    if cats.is_empty() {
        None
    } else {
        Some(cats)
    }
}

pub async fn get_events(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EventsQueryParams>,
) -> Result<Json<Value>, StatusCode> {
    let result = state
        .events
        .query(EventQuery {
            limit: q.limit,
            offset: q.offset,
            category: q.category,
            action: q.action,
            src_ip: q.src_ip,
            site_id: q.site_id,
            layer: q.layer,
            exclude_categories: parse_exclude_categories(q.exclude_category.as_deref()),
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let events: Value = if q.triage {
        let triages = triage_many(&state, &result.events).await;
        let rows: Vec<Value> = result
            .events
            .iter()
            .zip(triages.iter())
            .map(|(ev, t)| {
                let mut v = serde_json::to_value(ev).unwrap_or(json!({}));
                let mut triage_json = triage_to_json(t);
                if let Some(ai) = state.triage_ai.get_cached(&ev.id) {
                    crate::triage_ai::TriageAiService::merge_ai_into_json(&mut triage_json, &ai);
                }
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("triage".into(), triage_json);
                }
                v
            })
            .collect();
        json!(rows)
    } else {
        json!(result.events)
    };

    Ok(Json(json!({
        "total": result.total,
        "events": events,
        "triage": q.triage,
        "backend": state.events.backend_name(),
    })))
}

pub async fn post_events_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Vec<WafEventRecord>>,
) -> Result<Json<Value>, StatusCode> {
    let n = state
        .events
        .insert_batch(&body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "inserted": n })))
}

pub async fn stats_overview(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let stats = state
        .events
        .stats_overview()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "stats": stats,
        "backend": state.events.backend_name(),
    })))
}

#[derive(Deserialize)]
pub struct StatsSinceQuery {
    pub since: Option<String>,
}

/// Nerve-alert 摘要 JOB 回调：返回与 SourceStatsOverview 兼容的 JSON
pub async fn internal_stats(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StatsSinceQuery>,
) -> Result<Json<nerve_alert_sdk::SourceStatsOverview>, StatusCode> {
    let since = q
        .since
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
    let stats = state
        .events
        .stats_since(since)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(nerve_alert_sdk::SourceStatsOverview {
        total_events: stats.total_events,
        blocked: stats.blocked,
        logged: stats.logged,
        by_category: stats
            .by_category
            .into_iter()
            .map(|c| nerve_alert_sdk::CategoryCount {
                category: c.category,
                count: c.count,
            })
            .collect(),
        ..Default::default()
    }))
}

#[derive(Deserialize)]
pub struct SiteQuery {
    #[serde(default)]
    pub site_id: String,
}

fn resolve_site_id(q: &SiteQuery) -> &str {
    let id = q.site_id.trim();
    if id.is_empty() {
        "default"
    } else {
        id
    }
}

pub async fn get_modules(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
) -> Result<Json<ProtectionModules>, StatusCode> {
    let site_id = resolve_site_id(&q).to_string();
    if let Err(e) = state.config.ensure_app_provisioned(&site_id).await {
        tracing::error!("ensure_app_provisioned site={site_id}: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    state
        .config
        .load_modules_for(&site_id)
        .await
        .map_err(|e| {
            tracing::error!("load_modules_for site={site_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
        .map(Json)
}

pub async fn get_global_modules(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProtectionModules>, StatusCode> {
    state
        .config
        .load_global_modules()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn put_global_modules(
    State(state): State<Arc<AppState>>,
    Json(modules): Json<ProtectionModules>,
) -> Result<Json<Value>, StatusCode> {
    state
        .config
        .save_global_modules(&modules, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.platform.sync_from_protection_modules(&modules);
    state.refresh_l7_from_store().await;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true })))
}

/// 发布全局默认防护模块并同步运行时（CC/Bot platform、L7 热加载）
pub async fn publish_global_modules(
    State(state): State<Arc<AppState>>,
    Json(modules): Json<ProtectionModules>,
) -> Result<Json<Value>, StatusCode> {
    state
        .config
        .save_global_modules(&modules, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.platform.sync_from_protection_modules(&modules);
    state.refresh_l7_from_store().await;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "published": true })))
}

pub async fn put_modules(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SiteQuery>,
    Json(modules): Json<ProtectionModules>,
) -> Result<Json<Value>, StatusCode> {
    let site_id = resolve_site_id(&q).to_string();
    state
        .config
        .save_modules_for(&site_id, &modules, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if site_id == "default" {
        state.platform.sync_from_protection_modules(&modules);
        state.refresh_l7_from_store().await;
    }
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({ "ok": true, "site_id": site_id })))
}

#[derive(Deserialize)]
pub struct InspectBody {
    /// 防护应用 ID（旁路 Gateway 必传；缺省为 `default`）
    #[serde(default)]
    pub app_id: Option<String>,
    pub method: Option<String>,
    pub host: Option<String>,
    pub uri: String,
    #[serde(default)]
    pub headers: Option<serde_json::Value>,
    pub body: Option<String>,
    pub src_ip: Option<String>,
    pub user_agent: Option<String>,
    pub response_body: Option<String>,
    #[serde(default)]
    pub persist: bool,
    /// inspect 入口来源：ebpf / http-bypass / agent-local 等
    #[serde(default)]
    pub source: Option<String>,
    /// 关联 trace（缺省用 WAF 请求 trace）
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub journey_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub parent_journey_id: Option<String>,
    #[serde(default)]
    pub pod: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub dst_port: Option<u16>,
}

fn pick_app_id(body: &InspectBody) -> Option<String> {
    if let Some(ref id) = body.app_id {
        let t = id.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(obj) = body.headers.as_ref().and_then(|v| v.as_object()) {
        for key in ["x-app-id", "X-App-Id", "X-App-ID"] {
            if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn headers_to_string(headers: &Option<serde_json::Value>) -> String {
    match headers {
        None => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    }
}

fn header_from_inspect(headers: &Option<serde_json::Value>, names: &[&str]) -> Option<String> {
    if let Some(obj) = headers.as_ref().and_then(|v| v.as_object()) {
        for name in names {
            if let Some(v) = obj.get(*name).and_then(|v| v.as_str()) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
            let lower = name.to_ascii_lowercase();
            if let Some(v) = obj.get(lower.as_str()).and_then(|v| v.as_str()) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    let raw = headers_to_string(headers);
    if raw.is_empty() {
        return None;
    }
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            for name in names {
                if key.eq_ignore_ascii_case(name) && !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn resolve_inspect_trace(req: &InspectBody, trace: &NerveTrace) -> RequestTrace {
    let tp = header_from_inspect(&req.headers, &[HEADER_TRACEPARENT, "Traceparent"]);
    let (tp_trace, tp_span) = tp
        .as_deref()
        .and_then(parse_traceparent)
        .unwrap_or_default();

    let ts = header_from_inspect(&req.headers, &[HEADER_TRACESTATE, "Tracestate"])
        .as_deref()
        .map(parse_tracestate)
        .unwrap_or_default();

    let nerve_trace_id = req
        .trace_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            header_from_inspect(
                &req.headers,
                &[HEADER_NERVE_TRACE_ID, HEADER_X_NERVE_TRACE_ID, HEADER_X_TRACE_ID],
            )
        })
        .or_else(|| (!tp_trace.is_empty()).then(|| tp_trace))
        .unwrap_or_else(|| trace.nerve_trace_id.clone());

    let journey_id = req
        .journey_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| header_from_inspect(&req.headers, &[HEADER_NERVE_JOURNEY_ID]))
        .or(ts.journey_id.clone())
        .or_else(|| trace.journey_id.clone());

    let session_id = req
        .session_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| header_from_inspect(&req.headers, &["Nerve-Session-Id", "X-Session-Id"]))
        .or_else(|| journey_id.clone());

    let parent_journey_id = req
        .parent_journey_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| header_from_inspect(&req.headers, &[HEADER_NERVE_PARENT_JOURNEY_ID]))
        .or(ts.parent_journey_id.clone())
        .or_else(|| trace.parent_journey_id.clone());

    let parent_span = header_from_inspect(&req.headers, &[HEADER_NERVE_SPAN_ID])
        .or_else(|| (!tp_span.is_empty()).then(|| tp_span));

    RequestTrace::new_waf(
        Some(nerve_trace_id),
        parent_span,
        header_from_inspect(&req.headers, &[HEADER_X_TRACE_ID]),
        journey_id,
        session_id,
        parent_journey_id,
        ts.site_id,
    )
}

fn challenge_token_from_request(headers_raw: &str, headers_json: &Option<serde_json::Value>) -> Option<String> {
    if let Some(obj) = headers_json.as_ref().and_then(|v| v.as_object()) {
        let header_lower = CHALLENGE_HEADER.to_lowercase();
        if let Some(t) = obj
            .get(CHALLENGE_HEADER)
            .or_else(|| obj.get(header_lower.as_str()))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(t.to_string());
        }
        if let Some(cookie) = obj
            .get("cookie")
            .or_else(|| obj.get("Cookie"))
            .and_then(|v| v.as_str())
        {
            if let Some(t) = ChallengeStore::extract_token("", Some(cookie)) {
                return Some(t);
            }
        }
    }
    ChallengeStore::extract_token(headers_raw, None)
}

fn pick_inspect_source(body: &InspectBody, fallback: &str) -> String {
    body.source
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// 与告警触发条件对齐。
pub(crate) fn should_alert_waf_event(record: &WafEventRecord) -> bool {
    record.action == "block"
        || record.action == "challenge"
        || record.severity == "high"
        || record.severity == "critical"
}

/// 拦截/挑战/高危命中必须落库；显式 persist 时记录 detect 命中。
fn should_persist_inspect_hit(persist: bool, record: &WafEventRecord, blocked: bool) -> bool {
    persist || blocked || should_alert_waf_event(record)
}

fn journey_report_enabled() -> bool {
    match std::env::var("NERVE_JOURNEY_REPORT").as_deref() {
        Ok("0") | Ok("false") | Ok("off") => false,
        _ => true,
    }
}

pub async fn execute_inspect(state: &AppState, req: InspectBody) -> Result<Value, String> {
    let trace = NerveTrace(nerve_waf_sdk::RequestTrace::new_waf(
        None, None, None, None, None, None, None,
    ));
    execute_inspect_traced(state, req, &trace, "agent-local").await
}

/// 统一 inspect 流水线。`inspect_backend` 标识入口：`http-bypass`（Gateway/APISIX → :8780）、`agent-local`（Unix Socket）。
pub async fn execute_inspect_traced(
    state: &AppState,
    req: InspectBody,
    trace: &NerveTrace,
    inspect_backend: &str,
) -> Result<Value, String> {
    let trace_ctx = resolve_inspect_trace(&req, trace);
    let inspect_source = pick_inspect_source(&req, inspect_backend);
    let site_id = state
        .platform
        .resolve_inspect_site_id(pick_app_id(&req).as_deref());
    let method = req.method.unwrap_or_else(|| "GET".into());
    let host = req.host.unwrap_or_default();
    let uri = req.uri.clone();
    let headers = headers_to_string(&req.headers);
    let body = req.body.unwrap_or_default();
    let src_ip = req.src_ip.clone().unwrap_or_else(|| "127.0.0.1".into());
    let user_agent = req.user_agent.unwrap_or_default();
    let persist = req.persist;
    let response_body = req.response_body.clone();

    if journey_report_enabled() {
        let bind_host = if host.is_empty() { "upstream" } else { host.as_str() };
        let mut bind_trace = trace_ctx.clone();
        if bind_trace.site_id.is_none() {
            bind_trace.site_id = Some(site_id.clone());
        }
        let bind_body = crate::trace_bind::build_trace_bind_v2(
            &bind_trace,
            &src_ip,
            bind_host,
            req.dst_port.unwrap_or(80),
            &site_id,
            req.pod.as_deref(),
            req.namespace.as_deref(),
        );
        if let Err(e) = state.nerve.bind_trace_context(&bind_body).await {
            tracing::warn!(
                trace_id = %trace_ctx.nerve_trace_id,
                "trace bind sync failed: {e}"
            );
        }
    }

    let generation = crate::dataplane::dataplane_generation(state);
    let bot_challenge_verified = challenge_token_from_request(&headers, &req.headers)
        .map(|token| state.platform.challenge_store().verify(&token))
        .unwrap_or(false);

    let embed_req = EmbedInspectRequest {
        method: method.clone(),
        host: host.clone(),
        uri: uri.clone(),
        headers: headers.clone(),
        body: body.clone(),
        src_ip: src_ip.clone(),
        user_agent: user_agent.clone(),
        response_body: response_body.clone(),
    };

    let verdict = state
        .bypass_inspect
        .inspect_verdict(
            state,
            &site_id,
            generation,
            embed_req,
            bot_challenge_verified,
            Some(trace_ctx.nerve_trace_id.clone()),
        )
        .await?;

    let l7_req = InspectRequest {
        method,
        host,
        uri,
        headers,
        body,
        src_ip,
        user_agent,
    };

    let hit = verdict.hits.first();
    let blocked = matches!(
        verdict.action,
        nerve_waf_core::WafAction::Block | nerve_waf_core::WafAction::Challenge
    );
    let rule_id = hit.map(|h| h.rule_id.as_str()).unwrap_or("-");
    let rule_name = hit.map(|h| h.rule_name.as_str()).unwrap_or("-");

    logging::log_inspect(trace.merge(json!({
        "method": l7_req.method,
        "host": l7_req.host,
        "uri": l7_req.uri,
        "src_ip": l7_req.src_ip,
        "user_agent": l7_req.user_agent,
        "blocked": blocked,
        "allowed": verdict.allowed,
        "action": verdict.action.as_str(),
        "rule_id": rule_id,
        "rule_name": rule_name,
        "category": hit.map(|h| h.category.as_str()).unwrap_or("-"),
        "source": inspect_source,
    })));

    if blocked {
        logging::log_audit(trace.merge(json!({
            "method": l7_req.method,
            "uri": l7_req.uri,
            "src_ip": l7_req.src_ip,
            "action": verdict.action.as_str(),
            "rule_id": rule_id,
            "rule_name": rule_name,
            "category": hit.map(|h| h.category.as_str()).unwrap_or("-"),
            "severity": hit.map(|h| h.severity.as_str()).unwrap_or("-"),
            "payload_sample": hit.map(|h| h.payload_sample.as_str()).unwrap_or(""),
            "source": inspect_source,
        })));
    }

    if blocked
        && state
            .platform
            .should_auto_deny(
                hit.map(|h| h.category.as_str()).unwrap_or("-"),
                hit.map(|h| h.severity.as_str()).unwrap_or("medium"),
            )
    {
        if let Err(e) = state.nerve.deny_ip(&l7_req.src_ip).await {
            tracing::warn!("auto_deny ip={} failed: {e}", l7_req.src_ip);
        }
    }

    let mut event_id = None;
    let mut persisted = false;
    let mut triage_json = None;
    if let Some(h) = hit {
        let (strategy_id, strategy_name) = resolve_event_strategy(state, &site_id).await;
        let record = pipeline_hit_to_event(
            &l7_req,
            h,
            &site_id,
            trace,
            &inspect_source,
            strategy_id,
            strategy_name,
        );
        let need_persist = should_persist_inspect_hit(persist, &record, blocked);
        if need_persist {
            match state.events.insert_batch(&[record.clone()]).await {
                Ok(_) => {
                    event_id = Some(record.id.clone());
                    persisted = true;
                }
                Err(e) => {
                    tracing::error!(
                        "waf event persist failed: {e} action={} category={} uri={}",
                        record.action,
                        record.category,
                        record.uri.as_deref().unwrap_or("-")
                    );
                }
            }
        }
        let triage = triage_one(state, &record).await;
        let mut tj = triage_to_json(&triage);
        if let Some(ai) = state.triage_ai.get_cached(&record.id) {
            crate::triage_ai::TriageAiService::merge_ai_into_json(&mut tj, &ai);
        }
        triage_json = Some(tj);
        state.triage_ai.spawn_background(record.clone(), triage.clone());
        if should_alert_waf_event(&record) {
            if persisted || !need_persist {
                spawn_alert_dispatch(state, &record, Some(triage));
            } else {
                tracing::warn!(
                    "alert skipped: persist failed event={} uri={}",
                    record.id,
                    record.uri.as_deref().unwrap_or("-")
                );
            }
        }
    }

    if journey_report_enabled() {
        let ctx = WafJourneyContext {
            trace_id: Some(trace_ctx.nerve_trace_id.clone()),
            journey_id: trace_ctx.journey_id.clone(),
            session_id: trace_ctx.session_id.clone().or(trace_ctx.journey_id.clone()),
            parent_journey_id: trace_ctx.parent_journey_id.clone(),
            site_id: site_id.clone(),
            src_ip: l7_req.src_ip.clone(),
            dst_host: if l7_req.host.is_empty() {
                "upstream".into()
            } else {
                l7_req.host.clone()
            },
            dst_port: req.dst_port.unwrap_or(80),
            method: l7_req.method.clone(),
            uri: l7_req.uri.clone(),
            pod: req.pod.clone(),
            namespace: req.namespace.clone(),
            source_detail: inspect_source.clone(),
        };
        let nerve = state.nerve.clone();
        let verdict_for_journey = verdict.clone();
        tokio::spawn(async move {
            let hops = build_waf_journey_hops(&ctx, &verdict_for_journey);
            if hops.is_empty() {
                return;
            }
            if let Err(e) = nerve.post_journey_hops(&hops).await {
                tracing::debug!("waf journey hops batch: {e}");
            }
        });
    }

    let hit_json = hit.map(|h| {
        json!({
            "phase": h.phase,
            "action": h.action.as_str(),
            "category": h.category,
            "rule_id": h.rule_id,
            "rule_name": h.rule_name,
            "severity": h.severity,
            "matched_field": h.matched_field,
            "payload_sample": h.payload_sample,
            "plugin_name": h.plugin_name,
            "plugin_type": h.plugin_type,
            "param_path": h.param_path,
            "decode_chain": h.decode_chain,
            "semantic_score": h.semantic_score,
            "engines": h.engines,
        })
    });

    let timing = verdict.timing.clone();
    Ok(json!({
        "blocked": !verdict.allowed,
        "allowed": verdict.allowed,
        "action": verdict.action.as_str(),
        "site_id": site_id,
        "app_id": site_id,
        "verdict": verdict,
        "timing": timing,
        "hit": hit_json,
        "triage": triage_json,
        "eventId": event_id,
        "persisted": persisted,
        "backend": state.events.backend_name(),
        "source": inspect_source,
        "inspect_backend": inspect_backend,
        "trace": {
            "trace_id": trace_ctx.nerve_trace_id,
            "span_id": trace_ctx.nerve_span_id,
            "journey_id": trace_ctx.journey_id,
            "session_id": trace_ctx.session_id,
            "parent_journey_id": trace_ctx.parent_journey_id,
            "site_id": trace_ctx.site_id,
            "traceparent": trace_ctx.w3c_traceparent(),
            "tracestate": trace_ctx.w3c_tracestate(),
        },
    }))
}

/// HTTP 旁路检测入口（Gateway / APISIX 插件默认调用 `:8780/api/inspect`）。
pub async fn post_inspect(
    State(state): State<Arc<AppState>>,
    trace: NerveTrace,
    Json(req): Json<InspectBody>,
) -> Result<Json<Value>, StatusCode> {
    execute_inspect_traced(&state, req, &trace, "http-bypass")
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("inspect failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn poll_nerve_events(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let records = match collect_nerve_events(&state).await {
        Ok(r) => r,
        Err(e) => {
            return Ok(Json(json!({ "ok": false, "error": e.to_string(), "inserted": 0 })));
        }
    };
    let n = state
        .events
        .insert_batch(&records)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for ev in &records {
        if should_alert_network_event(ev) {
            spawn_network_alert_dispatch(state.as_ref(), ev);
        }
    }

    Ok(Json(json!({
        "ok": true,
        "inserted": n,
        "collected": records.len(),
        "backend": state.events.backend_name(),
    })))
}

async fn collect_nerve_events(state: &AppState) -> Result<Vec<WafEventRecord>, nerve_waf_client::NerveClientError> {
    let mut out = Vec::new();

    let audit = state.nerve.get_audit_export(200).await?;
    if let Some(arr) = audit.get("records").and_then(|v| v.as_array()) {
        for rec in arr {
            out.push(audit_to_event(rec));
        }
    }

    let anomalies = state.nerve.get_anomalies(100).await?;
    if let Some(arr) = anomalies.get("anomalies").and_then(|v| v.as_array()) {
        for rec in arr {
            out.push(anomaly_to_event(rec));
        }
    }

    let hits = state.nerve.get_policy_hits(100).await?;
    if let Some(arr) = hits.get("hits").and_then(|v| v.as_array()) {
        for rec in arr {
            out.push(policy_hit_to_event(rec));
        }
    }

    Ok(out)
}

fn audit_to_event(rec: &Value) -> WafEventRecord {
    let kind = rec.get("kind").and_then(|v| v.as_str()).unwrap_or("audit");
    let action = rec.get("action").and_then(|v| v.as_str()).unwrap_or("log");
    let src_ip = rec.get("src_ip").and_then(|v| v.as_str()).map(str::to_string);
    let dst_ip = rec.get("dst_ip").and_then(|v| v.as_str()).map(str::to_string);
    let message = rec.get("message").and_then(|v| v.as_str()).map(str::to_string);
    // policy_deny 历史数据可能只有 dst_ip；统一用于展示
    let display_ip = src_ip.clone().or(dst_ip.clone());
    let payload_sample = match kind {
        "policy_deny" => display_ip
            .as_ref()
            .map(|ip| format!("deny_ip={ip}")),
        "policy_hit" => message.clone(),
        _ => message.clone(),
    };
    WafEventRecord {
        id: stable_id(&format!("audit:{rec}")),
        ts: parse_time(rec.get("time")),
        site_id: "default".into(),
        layer: "l3".into(),
        action: if action == "deny" { "block".into() } else { action.into() },
        category: map_kind_category(kind),
        rule_id: None,
        rule_name: Some(kind.into()),
        severity: "medium".into(),
        src_ip: display_ip,
        dst_ip,
        dst_port: None,
        method: None,
        host: None,
        uri: None,
        user_agent: None,
        headers: None,
        body: None,
        payload_sample,
        matched_field: None,
        route_id: None,
        pod: rec.get("pod").and_then(|v| v.as_str()).map(str::to_string),
        message,
        trace_id: None,
        span_id: None,
        inspect_source: None,
        strategy_id: None,
        strategy_name: None,
        protection_stage: None,
        rule_set: None,
        plugin_name: None,
    }
}

fn anomaly_to_event(rec: &Value) -> WafEventRecord {
    let typ = rec.get("type").and_then(|v| v.as_str()).unwrap_or("anomaly");
    let mut message = rec.get("message").and_then(|v| v.as_str()).map(str::to_string);
    if let Some(signals) = rec.get("signals").and_then(|v| v.as_array()) {
        let parts: Vec<String> = signals
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !parts.is_empty() {
            let combo = format!("signals={}", parts.join("+"));
            message = Some(match message {
                Some(m) => format!("{combo} · {m}"),
                None => combo,
            });
        }
    }
    WafEventRecord {
        id: stable_id(&format!("anomaly:{rec}")),
        ts: parse_time(rec.get("time")),
        site_id: "default".into(),
        layer: "l3".into(),
        action: rec
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("log")
            .into(),
        category: typ.into(),
        rule_id: None,
        rule_name: Some(typ.into()),
        severity: rec
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .into(),
        src_ip: rec.get("src_ip").and_then(|v| v.as_str()).map(str::to_string),
        dst_ip: rec.get("dst_ip").and_then(|v| v.as_str()).map(str::to_string),
        dst_port: rec.get("dport").and_then(|v| v.as_u64()).map(|p| p as u16),
        method: None,
        host: None,
        uri: rec.get("access_addr").and_then(|v| v.as_str()).map(str::to_string),
        user_agent: None,
        headers: None,
        body: None,
        payload_sample: rec.get("message").and_then(|v| v.as_str()).map(str::to_string),
        matched_field: None,
        route_id: None,
        pod: rec.get("pod").and_then(|v| v.as_str()).map(str::to_string),
        message,
        trace_id: None,
        span_id: None,
        inspect_source: None,
        strategy_id: None,
        strategy_name: None,
        protection_stage: None,
        rule_set: None,
        plugin_name: None,
    }
}

fn policy_hit_to_event(rec: &Value) -> WafEventRecord {
    WafEventRecord {
        id: stable_id(&format!("hit:{rec}")),
        ts: parse_time(rec.get("time")),
        site_id: "default".into(),
        layer: "l3".into(),
        action: "block".into(),
        category: "l3_policy".into(),
        rule_id: Some("policy_hit".into()),
        rule_name: Some("TC Policy Hit".into()),
        severity: "high".into(),
        src_ip: rec.get("src_ip").and_then(|v| v.as_str()).map(str::to_string),
        dst_ip: None,
        dst_port: rec.get("dport").and_then(|v| v.as_u64()).map(|p| p as u16),
        method: None,
        host: None,
        uri: None,
        user_agent: None,
        headers: None,
        body: None,
        payload_sample: None,
        matched_field: None,
        route_id: None,
        pod: rec.get("pod").and_then(|v| v.as_str()).map(str::to_string),
        message: Some("TC deny packet".into()),
        trace_id: None,
        span_id: None,
        inspect_source: None,
        strategy_id: None,
        strategy_name: None,
        protection_stage: None,
        rule_set: None,
        plugin_name: None,
    }
}

async fn resolve_event_strategy(
    state: &AppState,
    site_id: &str,
) -> (Option<String>, Option<String>) {
    let sid = match state.config.get_app_bound_strategy(site_id).await {
        Ok(id) if !id.is_empty() => id,
        _ => return (None, None),
    };
    let name = match state.config.get_protection_strategy(&sid).await {
        Ok(Some(tpl)) => Some(tpl.name),
        _ => None,
    };
    (Some(sid), name)
}

fn pipeline_hit_to_event(
    req: &InspectRequest,
    hit: &nerve_waf_core::InspectHit,
    site_id: &str,
    trace: &nerve_waf_sdk::RequestTrace,
    inspect_source: &str,
    strategy_id: Option<String>,
    strategy_name: Option<String>,
) -> WafEventRecord {
    WafEventRecord {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now(),
        site_id: site_id.into(),
        layer: "l7".into(),
        action: hit.action.as_str().into(),
        category: hit.category.clone(),
        rule_id: Some(hit.rule_id.clone()),
        rule_name: Some(hit.rule_name.clone()),
        severity: hit.severity.clone(),
        src_ip: Some(req.src_ip.clone()),
        dst_ip: None,
        dst_port: None,
        method: Some(req.method.clone()),
        host: Some(req.host.clone()),
        uri: Some(req.uri.clone()),
        user_agent: Some(req.user_agent.clone()),
        headers: Some(truncate_string(&req.headers, MAX_HEADERS_LEN)),
        body: Some(truncate_string(&req.body, MAX_BODY_LEN)),
        payload_sample: Some(hit.payload_sample.clone()),
        matched_field: Some(hit.matched_field.clone()),
        route_id: None,
        pod: None,
        message: semantic_event_message(hit),
        trace_id: Some(trace.nerve_trace_id.clone()),
        span_id: Some(trace.nerve_span_id.clone()),
        inspect_source: Some(inspect_source.to_string()),
        strategy_id,
        strategy_name,
        protection_stage: hit.protection_stage.clone(),
        rule_set: hit.rule_set.clone(),
        plugin_name: hit.plugin_name.clone(),
    }
}

const MAX_HEADERS_LEN: usize = 4096;
const MAX_BODY_LEN: usize = 8192;

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut end = max_len;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}... (truncated)", &s[..end])
}

fn semantic_event_message(hit: &nerve_waf_core::InspectHit) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(p) = &hit.param_path {
        parts.push(format!("param={p}"));
    }
    if let Some(s) = hit.semantic_score {
        parts.push(format!("semantic_score={s}"));
    }
    if let Some(chain) = &hit.decode_chain {
        parts.push(format!("decode={}", chain.join(">")));
    }
    if let Some(eng) = &hit.engines {
        parts.push(format!("engines={}", eng.join(",")));
    }
    if let Some(p) = &hit.plugin_name {
        parts.push(format!("plugin={p}"));
    }
    if parts.is_empty() {
        hit.plugin_name.as_ref().map(|p| format!("plugin={p}"))
    } else {
        Some(parts.join(" "))
    }
}

fn inspect_hit_to_event(req: &InspectRequest, hit: &nerve_waf_l7::InspectResult) -> WafEventRecord {
    WafEventRecord {
        id: Uuid::new_v4().to_string(),
        ts: Utc::now(),
        site_id: "default".into(),
        layer: "l7".into(),
        action: hit.action.clone(),
        category: hit.category.clone(),
        rule_id: Some(hit.rule_id.clone()),
        rule_name: Some(hit.rule_name.clone()),
        severity: hit.severity.clone(),
        src_ip: Some(req.src_ip.clone()),
        dst_ip: None,
        dst_port: None,
        method: Some(req.method.clone()),
        host: Some(req.host.clone()),
        uri: Some(req.uri.clone()),
        user_agent: Some(req.user_agent.clone()),
        headers: Some(truncate_string(&req.headers, MAX_HEADERS_LEN)),
        body: Some(truncate_string(&req.body, MAX_BODY_LEN)),
        payload_sample: Some(hit.payload_sample.clone()),
        matched_field: Some(hit.matched_field.clone()),
        route_id: None,
        pod: None,
        message: None,
        trace_id: None,
        span_id: None,
        inspect_source: None,
        strategy_id: None,
        strategy_name: None,
        protection_stage: None,
        rule_set: None,
        plugin_name: None,
    }
}

pub fn payload_from_event(ev: &WafEventRecord) -> AlertPayload {
    AlertPayload {
        product: WAF.into(),
        tenant: "default".into(),
        ts: ev.ts,
        severity: ev.severity.clone(),
        category: ev.category.clone(),
        action: ev.action.clone(),
        src_ip: ev.src_ip.clone(),
        host: ev.host.clone(),
        uri: ev.uri.clone(),
        message: ev.message.clone().unwrap_or_else(|| ev.category.clone()),
        site_id: Some(ev.site_id.clone()),
        trace_id: ev.trace_id.clone(),
        span_id: ev.span_id.clone(),
    }
}

pub(crate) fn spawn_alert_dispatch(
    state: &AppState,
    ev: &WafEventRecord,
    triage: Option<nerve_waf_triage::TriageResult>,
) {
    if !should_alert_waf_event(ev) {
        return;
    }
    if let Some(ref t) = triage {
        if should_suppress_alert(t) {
            tracing::info!(
                "alert suppressed by triage verdict={:?} event={}",
                t.verdict,
                ev.id
            );
            return;
        }
    }
    let mut payload = payload_from_event(ev);
    if let Some(t) = triage {
        payload.message = format!("{}\n{}", payload.message, triage_alert_line(&t));
    }
    state.alert_client.spawn_dispatch(DispatchRequest {
        product: WAF.into(),
        tenant: "default".into(),
        trigger: "realtime".into(),
        event_id: Some(ev.id.clone()),
        test: false,
        event: payload,
    });
}

/// L3 网络观测：仅组合信号 / 高危 / 拦截 才推送告警，plain orphan 不告警。
pub fn should_alert_network_event(ev: &WafEventRecord) -> bool {
    if ev.layer != "l3" {
        return false;
    }
    if ev.action == "block" {
        return true;
    }
    match ev.severity.as_str() {
        "critical" | "high" => true,
        "low" | "info" => false,
        _ => {
            !matches!(
                ev.category.as_str(),
                "orphan_connect" | "anomaly"
            )
        }
    }
}

fn spawn_network_alert_dispatch(state: &AppState, ev: &WafEventRecord) {
    state.alert_client.spawn_dispatch(DispatchRequest {
        product: WAF.into(),
        tenant: "default".into(),
        trigger: "network".into(),
        event_id: Some(ev.id.clone()),
        test: false,
        event: payload_from_event(ev),
    });
}

fn map_kind_category(kind: &str) -> String {
    match kind {
        "policy_hit" | "policy_deny" => "l3_policy".into(),
        "anomaly" => "anomaly".into(),
        _ => kind.into(),
    }
}

fn parse_time(v: Option<&Value>) -> DateTime<Utc> {
    v.and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn stable_id(seed: &str) -> String {
    use uuid::{uuid, Uuid};
    Uuid::new_v5(&uuid!("f47ac10b-58cc-4372-a567-0e02b2c3d479"), seed.as_bytes()).to_string()
}

pub fn spawn_nerve_poller(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Ok(records) = collect_nerve_events(&state).await {
                if !records.is_empty() {
                    if let Err(e) = state.events.insert_batch(&records).await {
                        tracing::warn!("nerve poll insert failed: {e}");
                    }
                }
            }
        }
    });
}
