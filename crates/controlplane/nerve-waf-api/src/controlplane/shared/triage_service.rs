//! 智能研判服务 — 供 inspect / 事件列表 / 告警 自动调用。

use nerve_waf_sdk::{EventQuery, WafEventRecord};
use nerve_waf_triage::{analyze_event, TriageContext, TriageResult, TriageVerdict};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::AppState;

pub async fn triage_one(state: &AppState, event: &WafEventRecord) -> TriageResult {
    let ctx = build_context(state, event).await;
    analyze_event(event, &ctx)
}

pub async fn triage_many(state: &AppState, events: &[WafEventRecord]) -> Vec<TriageResult> {
    if events.is_empty() {
        return vec![];
    }
    let ip_stats = batch_ip_stats(state, events).await;
    let modules = state
        .config
        .load_modules()
        .await
        .unwrap_or_else(|_| nerve_waf_sdk::ProtectionModules::default());
    let intel = state.platform.intel_feed();
    let global_mode = modules.mode;

    events
        .iter()
        .map(|ev| {
            let ctx = context_from_stats(ev, &ip_stats, &intel, &global_mode);
            analyze_event(ev, &ctx)
        })
        .collect()
}

pub fn triage_to_json(t: &TriageResult) -> Value {
    json!({
        "verdict": t.verdict,
        "confidence": t.confidence,
        "risk_score": t.risk_score,
        "summary": t.summary,
        "evidence": t.evidence,
        "recommendations": t.recommendations,
        "engine": t.engine,
    })
}

pub fn triage_alert_line(t: &TriageResult) -> String {
    format!(
        "[研判:{} conf={}% risk={}] {}",
        verdict_label(&t.verdict),
        t.confidence,
        t.risk_score,
        t.summary
    )
}

pub fn should_suppress_alert(t: &TriageResult) -> bool {
    matches!(t.verdict, TriageVerdict::LikelyFalsePositive)
}

fn verdict_label(v: &TriageVerdict) -> &'static str {
    match v {
        TriageVerdict::ConfirmedAttack => "确认攻击",
        TriageVerdict::LikelyAttack => "疑似攻击",
        TriageVerdict::NeedsReview => "待复核",
        TriageVerdict::LikelyFalsePositive => "疑似误报",
    }
}

struct IpStats {
    event_count: u32,
    blocked_count: u32,
}

async fn batch_ip_stats(
    state: &AppState,
    events: &[WafEventRecord],
) -> HashMap<String, IpStats> {
    let mut map: HashMap<String, IpStats> = HashMap::new();
    for ev in events {
        if let Some(ip) = ev.src_ip.as_ref().filter(|s| !s.is_empty()) {
            map.entry(ip.clone()).or_insert(IpStats {
                event_count: 0,
                blocked_count: 0,
            });
        }
    }
    for ip in map.keys().cloned().collect::<Vec<_>>() {
        if let Ok(recent) = state
            .events
            .query(EventQuery {
                limit: 200,
                src_ip: Some(ip.clone()),
                ..Default::default()
            })
            .await
        {
            let mut blocked = 0u32;
            for e in &recent.events {
                if e.action == "block" {
                    blocked += 1;
                }
            }
            if let Some(s) = map.get_mut(&ip) {
                s.event_count = recent.events.len() as u32;
                s.blocked_count = blocked;
            }
        }
    }
    map
}

async fn build_context(state: &AppState, event: &WafEventRecord) -> TriageContext {
    let modules = state
        .config
        .load_modules()
        .await
        .unwrap_or_else(|_| nerve_waf_sdk::ProtectionModules::default());

    let src_ip = event.src_ip.as_deref().unwrap_or("");
    let mut ip_event_count = 0u32;
    let mut ip_blocked_count = 0u32;

    if !src_ip.is_empty() {
        if let Ok(recent) = state
            .events
            .query(EventQuery {
                limit: 200,
                src_ip: Some(src_ip.into()),
                ..Default::default()
            })
            .await
        {
            ip_event_count = recent.events.len() as u32;
            ip_blocked_count = recent
                .events
                .iter()
                .filter(|e| e.action == "block")
                .count() as u32;
        }
    }

    let intel = state.platform.intel_feed();
    TriageContext {
        ip_event_count,
        ip_blocked_count,
        intel_malicious: !src_ip.is_empty() && intel.is_malicious(src_ip),
        intel_category: intel.category(src_ip).map(|s| s.to_string()),
        global_mode: modules.mode,
    }
}

fn context_from_stats(
    event: &WafEventRecord,
    stats: &HashMap<String, IpStats>,
    intel: &nerve_waf_intel::IntelFeed,
    global_mode: &str,
) -> TriageContext {
    let src_ip = event.src_ip.as_deref().unwrap_or("");
    let (ip_event_count, ip_blocked_count) = stats
        .get(src_ip)
        .map(|s| (s.event_count, s.blocked_count))
        .unwrap_or((0, 0));

    TriageContext {
        ip_event_count,
        ip_blocked_count,
        intel_malicious: !src_ip.is_empty() && intel.is_malicious(src_ip),
        intel_category: intel.category(src_ip).map(|s| s.to_string()),
        global_mode: global_mode.to_string(),
    }
}
