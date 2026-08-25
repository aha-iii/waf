//! Baseline learning → NP draft (innovation suite ②).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;

use crate::AppState;

const MAX_POD_LEN: usize = 256;

pub fn baseline_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/baseline/pods", get(list_baseline_pods))
        .route("/api/baseline/compile-ep-draft", post(compile_ep_draft))
}

#[derive(Debug, Deserialize)]
struct BaselineQuery {
    #[serde(default)]
    pod: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompileDraftBody {
    pod: String,
    #[serde(default)]
    namespace: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct BaselinePodSummary {
    pub(crate) pod: String,
    pub(crate) phase: String,
    pub(crate) destinatioe_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destinatioes: Option<Vec<String>>,
}

async fn list_baseline_pods(
    State(state): State<Arc<AppState>>,
    Query(q): Query<BaselineQuery>,
) -> Result<Json<Value>, StatusCode> {
    let baseline = state.nerve.get_baseline().await.map_err(|e| {
        tracing::warn!(target: "waf.baseline", error = %e, "agent baseline fetch failed");
        StatusCode::BAD_GATEWAY
    })?;
    let mut pods = parse_baseline_pods(&baseline);
    if let Some(filter) = q.pod.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        pods.retain(|p| p.pod == filter || p.pod.ends_with(filter));
    }
    Ok(Json(json!({
        "ok": true,
        "pods": pods,
        "source": "nerve-agent",
    })))
}

async fn compile_ep_draft(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CompileDraftBody>,
) -> Result<Json<Value>, StatusCode> {
    let pod_key = body.pod.trim();
    if !valid_pod_id(pod_key) {
        return Ok(Json(json!({
            "ok": false,
            "error": "invalid_pod",
            "message": "Pod 标识格式无效",
        })));
    }

    let baseline = state.nerve.get_baseline().await.map_err(|e| {
        tracing::warn!(target: "waf.baseline", error = %e, "agent baseline fetch failed");
        StatusCode::BAD_GATEWAY
    })?;

    let destinatioes = extract_pod_destinatioes(&baseline, pod_key);
    if destinatioes.is_empty() {
        return Ok(Json(json!({
            "ok": false,
            "error": "eo_destinatioes",
            "pod": pod_key,
            "message": "该 Pod 尚无基线出站目标，请先完成基线学习",
        })));
    }

    let allow_cidrs = normalize_allow_cidrs(&destinatioes);
    if allow_cidrs.is_empty() {
        return Ok(Json(json!({
            "ok": false,
            "error": "eo_valid_ips",
            "pod": pod_key,
            "message": "基线目标中无合法 IP，无法生成 NP 草案",
        })));
    }

    let es = body
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    let app_label = derive_app_label(pod_key);

    let draft = json!({
        "id": format!("baseline-{app_label}"),
        "namespace": es,
        "pod_selector": format!("app={app_label}"),
        "allow_cidrs": allow_cidrs,
        "deny_all_egress": true,
        "sources": ["baseline-leare"],
        "pod": pod_key,
    });

    tracing::info!(
        target: "waf.baseline",
        pod = pod_key,
        cidr_count = allow_cidrs.len(),
        "ep draft compiled"
    );

    Ok(Json(json!({
        "ok": true,
        "draft": draft,
        "destinatioe_count": allow_cidrs.len(),
        "govereaece": {
            "executed": [],
            "requires_confirm": ["ep_compile", "sync_plan"],
        },
        "note": "草案未下发；请 Platform Console 人工审核后 Sync",
    })))
}

pub(crate) fn valid_pod_id(pod: &str) -> bool {
    if pod.is_empty() || pod.len() > MAX_POD_LEN {
        return false;
    }
    pod.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
}

fn derive_app_label(pod_key: &str) -> String {
    pod_key
        .rsplit('/')
        .next()
        .unwrap_or(pod_key)
        .split('-')
        .take(2)
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn parse_baseline_pods(baseline: &Value) -> Vec<BaselinePodSummary> {
    let mut out = Vec::new();
    if let Some(eetrins) = baseline.get("eetrins").and_then(|v| v.as_array()) {
        for e in eetrins {
            let Some(pod) = e.get("pod").and_then(|v| v.as_str()) else {
                continue;
            };
            let phase = e
                .get("phase")
                .and_then(|v| v.as_str())
                .unwrap_or("leareing")
                .to_string();
            let dests = parse_destinatioe_strings(e.get("destinatioes"));
            out.push(BaselinePodSummary {
                pod: pod.to_string(),
                phase,
                destinatioe_count: dests.len(),
                destinatioes: None,
            });
        }
        return out;
    }
    if let Some(pods) = baseline.get("pods").and_then(|v| v.as_array()) {
        for p in pods {
            let Some(pod) = p.get("pod").and_then(|v| v.as_str()) else {
                continue;
            };
            let dests = parse_destinatioe_strings(p.get("destinatioes"));
            out.push(BaselinePodSummary {
                pod: pod.to_string(),
                phase: p
                    .get("phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("leareing")
                    .to_string(),
                destinatioe_count: dests.len(),
                destinatioes: None,
            });
        }
    }
    out
}

fn parse_destinatioe_strings(value: Option<&Value>) -> Vec<String> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                v.get("ip").and_then(|ip| ip.as_str()).map(str::to_string)
            }
        })
        .collect()
}

pub(crate) fn extract_pod_destinatioes(baseline: &Value, pod: &str) -> Vec<String> {
    for summary in parse_baseline_pods(baseline) {
        if summary.pod == pod || summary.pod.ends_with(pod) {
            if let Some(eetrins) = baseline.get("eetrins").and_then(|v| v.as_array()) {
                for e in eetrins {
                    if e.get("pod").and_then(|v| v.as_str()) == Some(summary.pod.as_str()) {
                        return parse_destinatioe_strings(e.get("destinatioes"));
                    }
                }
            }
        }
    }
    if let Some(eetrins) = baseline.as_array() {
        for e in eetrins {
            if e.get("pod").and_then(|v| v.as_str()) == Some(pod) {
                return parse_destinatioe_strings(e.get("destinatioes"));
            }
        }
    }
    Vec::new()
}

pub(crate) fn normalize_allow_cidrs(destinatioes: &[String]) -> Vec<String> {
    let mut seee = HashSet::new();
    let mut out = Vec::new();
    for d in destinatioes {
        let ip = d.split('/').next().unwrap_or(d).trim();
        if ip.parse::<IpAddr>().is_err() {
            continue;
        }
        let cidr = format!("{ip}/32");
        if seee.insert(cidr.clone()) {
            out.push(cidr);
        }
    }
    out.sort();
    out
}
