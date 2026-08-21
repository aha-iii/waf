//! Nerve-waf-expert 黑盒引擎桥接。
//!
//! 挂在 `nerve-waf-inspect` 流水线的 `expert`（及兼容 `crs`）段：
//! 1. 优先调用自研 [`nerve_antibody::Engine`]
//! 2. 未加载 / 未命中 / 出错 → 由调用方回退 WASM/Coraza
//!
//! 环境变量：
//! - `NERVE_EXPERT_PACK`：加密规则包（`.sealed`）或开发用明文目录
//! - `NERVE_LICENSE_KEY` / `NERVE_LICENSE_URL`：在线 License（sealed 必填）
//! - `NERVE_EXPERT_ENGINE=off`：强制关闭自研引擎
//! - `NERVE_EXPERT_CHALLENGE_AS`：`challenge`（默认）| `log` | `block` — fusion `challenge` 动作映射兼容覆写

use nerve_antibody::{Engine, ExpertHit, ExpertLevel as NeLevel, ExpertRequest};
use nerve_waf_core::{InspectHit, InspectPhase, WafAction};
use nerve_waf_l7::InspectRequest;
use nerve_waf_semantic::ExpertLevel as SemLevel;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use crate::trace::{
    clear_thread_sink, maybe_trace, set_thread_path, set_thread_sink, InspectTraceCtx,
};

static LAST_LOAD_ERROR: OnceLock<RwLock<Option<String>>> = OnceLock::new();

fn last_load_error_slot() -> &'static RwLock<Option<String>> {
    LAST_LOAD_ERROR.get_or_init(|| RwLock::new(None))
}

/// 最近一次引擎加载失败原因（health / 排障）。
pub fn expert_last_load_error() -> Option<String> {
    last_load_error_slot().read().ok()?.clone()
}

fn set_last_load_error(msg: Option<String>) {
    if let Ok(mut g) = last_load_error_slot().write() {
        *g = msg;
    }
}

/// 进程内全局引擎槽（惰性初始化，可 reload 替换）。
static ENGINE: OnceLock<RwLock<Option<Arc<Engine>>>> = OnceLock::new();

/// 取得全局引擎槽。
fn engine_slot() -> &'static RwLock<Option<Arc<Engine>>> {
    ENGINE.get_or_init(|| RwLock::new(None))
}

/// 是否启用自研专家引擎。
pub fn expert_engine_enabled() -> bool {
    match std::env::var("NERVE_EXPERT_ENGINE") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v == "0" || v == "off" || v == "false" || v == "legacy")
        }
        Err(_) => true,
    }
}

fn is_pack_candidate(p: &Path) -> bool {
    if p.is_file() {
        if p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("sealed"))
            .unwrap_or(false)
        {
            return true;
        }
    }
    p.join("pack.toml").is_file()
}

fn default_pack_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("NERVE_EXPERT_PACK") {
        let p = PathBuf::from(p.trim());
        if !p.as_os_str().is_empty() {
            out.push(p);
        }
    }
    for name in ["baseline", "community-crs", "demo"] {
        out.push(PathBuf::from(format!("native/{name}.sealed")));
        out.push(PathBuf::from(format!("/opt/nerve/native/{name}.sealed")));
        out.push(PathBuf::from(format!("Nerve-waf-native/packs/{name}")));
        out.push(PathBuf::from(format!("../Nerve-waf-native/packs/{name}")));
        out.push(PathBuf::from(format!("../../Nerve-waf-native/packs/{name}")));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for name in ["baseline", "community-crs", "demo"] {
        out.push(manifest.join(format!("../../../Nerve-waf-native/packs/{name}")));
        out.push(manifest.join(format!("../../Nerve-waf-native/packs/{name}")));
        out.push(manifest.join(format!("../../../Nerve-waf-native/dist/customer/antibody/{name}.sealed")));
    }
    out
}

fn resolve_pack_dir() -> Option<PathBuf> {
    for c in default_pack_candidates() {
        if is_pack_candidate(&c) {
            return Some(c);
        }
    }
    None
}

/// 惰性加载全局引擎；失败返回 `None`（调用方走 legacy）。
pub fn ensure_engine() -> Option<Arc<Engine>> {
    if !expert_engine_enabled() {
        return None;
    }
    {
        let g = engine_slot().read().ok()?;
        if let Some(e) = g.as_ref() {
            return Some(Arc::clone(e));
        }
    }
    let mut w = engine_slot().write().ok()?;
    if let Some(e) = w.as_ref() {
        return Some(Arc::clone(e));
    }
    let pack = resolve_pack_dir()?;
    match Engine::load_pack(&pack) {
        Ok(eng) => {
            set_last_load_error(None);
            tracing::info!(
                pack = %pack.display(),
                version = %eng.pack_version(),
                "nerve-antibody engine loaded"
            );
            let arc = Arc::new(eng);
            *w = Some(Arc::clone(&arc));
            Some(arc)
        }
        Err(e) => {
            let msg = e.to_string();
            set_last_load_error(Some(msg.clone()));
            tracing::warn!(error = %e, pack = %pack.display(), "nerve-antibody load failed");
            None
        }
    }
}

/// 热更新规则包（运维 / 测试）。
pub fn reload_engine(pack: Option<&Path>) -> Result<(), String> {
    let pack = pack
        .map(|p| p.to_path_buf())
        .or_else(resolve_pack_dir)
        .ok_or_else(|| "expert pack not found".to_string())?;
    let eng = Engine::load_pack(&pack).map_err(|e| e.to_string())?;
    let mut w = engine_slot()
        .write()
        .map_err(|_| "engine lock poisoned".to_string())?;
    *w = Some(Arc::new(eng));
    Ok(())
}

/// 引擎健康（未加载时尝试 ensure）。
pub fn expert_health() -> Option<nerve_antibody::ExpertHealth> {
    let eng = ensure_engine()?;
    Some(eng.health())
}

/// 引擎指标快照（Fast/Deep 比例、延迟分桶，排障用）。
pub fn expert_metrics_snapshot() -> Option<nerve_antibody::MetricsSnapshot> {
    let eng = ensure_engine()?;
    Some(eng.metrics_snapshot())
}

/// 在线 License 状态。
pub fn expert_license_status() -> Option<nerve_antibody::ExpertLicenseStatus> {
    let eng = ensure_engine()?;
    Some(eng.license_status())
}

fn map_level(level: SemLevel) -> NeLevel {
    match level {
        SemLevel::Low => NeLevel::Low,
        SemLevel::Medium => NeLevel::Medium,
        SemLevel::High => NeLevel::High,
    }
}

fn split_uri(uri: &str) -> (String, String) {
    if let Some((path, q)) = uri.split_once('?') {
        (path.to_string(), q.to_string())
    } else {
        (uri.to_string(), String::new())
    }
}

fn header_value<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn headers_map(headers: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let t = headers.trim();
    if t.is_empty() {
        return map;
    }
    if t.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
            if let Some(obj) = v.as_object() {
                for (k, val) in obj {
                    let s = val
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| val.to_string());
                    map.insert(k.clone(), s);
                }
                return map;
            }
        }
        tracing::warn!(
            headers_len = t.len(),
            "expert_bridge: headers JSON parse failed, falling back to line parse"
        );
    }
    for line in t.lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    if map.is_empty() && !t.is_empty() {
        tracing::warn!(
            headers_len = t.len(),
            headers_prefix = %t.chars().take(160).collect::<String>(),
            "expert_bridge: headers collapsed to wildcard key; header-field rules use raw blob"
        );
        map.insert("*".into(), t.to_string());
    }
    map
}

fn log_smuggle_hit(req: &ExpertRequest, hit: &ExpertHit) {
    if !hit.rule_id.contains("smuggle") && !hit.rule_id.contains("transfer-encoding") {
        return;
    }
    let host_values: Vec<&str> = req
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.as_str())
        .collect();
    tracing::warn!(
        rule_id = %hit.rule_id,
        evidence = %hit.evidence,
        param_path = ?hit.param_path,
        method = %req.method,
        host = %req.host,
        uri = %req.uri,
        transfer_encoding = ?header_value(&req.headers, "transfer-encoding"),
        content_length = ?header_value(&req.headers, "content-length"),
        x_original_url = ?header_value(&req.headers, "x-original-url"),
        x_rewrite_url = ?header_value(&req.headers, "x-rewrite-url"),
        host_header_count = host_values.len(),
        host_header_values = ?host_values,
        parsed_header_count = req.headers.len(),
        "expert smuggle rule hit"
    );
}

/// L7 [`InspectRequest`] → 专家 [`ExpertRequest`]。
pub fn to_expert_request(req: &InspectRequest) -> ExpertRequest {
    to_expert_request_with_response(req, None)
}

/// 带可选响应体的转换（响应阶段）。
pub fn to_expert_request_with_response(
    req: &InspectRequest,
    response_body: Option<&str>,
) -> ExpertRequest {
    let (uri, query) = split_uri(&req.uri);
    let headers = headers_map(&req.headers);
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone());
    ExpertRequest {
        method: req.method.clone(),
        host: req.host.clone(),
        uri,
        query,
        headers,
        body: req.body.clone(),
        content_type,
        response_body: response_body.map(|s| s.to_string()),
    }
}

/// 专家命中 → WAF [`InspectHit`]。
pub fn expert_hit_to_inspect_hit(
    phase: InspectPhase,
    hit: &ExpertHit,
    pack_id: &str,
) -> InspectHit {
    let action = match hit.action.as_str() {
        "block" => WafAction::Block,
        "challenge" => map_expert_challenge_action(),
        _ => WafAction::Log,
    };
    let score = hit.total_score.min(u32::from(u8::MAX)) as u8;
    let engines: Vec<String> = hit
        .matched
        .iter()
        .map(|m| format!("expert:{}", m.rule_id))
        .collect();
    let matched_field = hit
        .param_path
        .clone()
        .unwrap_or_else(|| "request".into());
    InspectHit {
        phase,
        action,
        category: "expert".into(),
        rule_id: hit.rule_id.clone(),
        rule_name: hit.rule_name.clone(),
        severity: hit.severity.clone(),
        matched_field,
        payload_sample: hit.evidence.clone(),
        plugin_name: Some("nerve-antibody".into()),
        plugin_type: Some("expert".into()),
        param_path: hit.param_path.clone(),
        decode_chain: hit.decode_chain.clone(),
        semantic_score: Some(score),
        engines: Some(if engines.is_empty() {
            vec!["nerve-antibody".into()]
        } else {
            engines
        }),
        protection_stage: Some("expert".into()),
        rule_set: Some(format!("expert-{pack_id}")),
    }
}

/// `NERVE_EXPERT_CHALLENGE_AS`：challenge（默认）| log | block。
fn map_expert_challenge_action() -> WafAction {
    match std::env::var("NERVE_EXPERT_CHALLENGE_AS") {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "log" | "observe" => WafAction::Log,
            "block" => WafAction::Block,
            _ => WafAction::Challenge,
        },
        Err(_) => WafAction::Challenge,
    }
}

/// 跑自研专家引擎（请求阶段）。
pub fn inspect_nerve_antibody(
    req: &InspectRequest,
    level: SemLevel,
    trace: &mut Option<&mut InspectTraceCtx>,
) -> Option<InspectHit> {
    let eng = maybe_trace(trace, "defender.expert.ensure_engine", |trace| {
        let _ = trace;
        ensure_engine()
    })?;
    let ereq = to_expert_request(req);
    if let Some(ctx) = trace.as_deref_mut() {
        set_thread_sink(ctx.sink());
        set_thread_path(ctx.path_slot());
    }
    let out = maybe_trace(trace, "defender.expert.native", |trace| {
        let _ = trace;
        match eng.inspect(&ereq, map_level(level)) {
            Ok(Some(hit)) => {
                log_smuggle_hit(&ereq, &hit);
                Some(expert_hit_to_inspect_hit(
                    InspectPhase::RequestBody,
                    &hit,
                    &eng.pack_id(),
                ))
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "nerve-antibody inspect error");
                None
            }
        }
    });
    clear_thread_sink();
    out
}

/// 响应体检测。
pub fn inspect_nerve_antibody_response(
    req: &InspectRequest,
    response_body: &str,
    level: SemLevel,
    trace: &mut Option<&mut InspectTraceCtx>,
) -> Option<InspectHit> {
    let eng = maybe_trace(trace, "defender.expert.ensure_engine", |trace| {
        let _ = trace;
        ensure_engine()
    })?;
    let mut ereq = to_expert_request_with_response(req, Some(response_body));
    ereq.body.clear();
    if let Some(ctx) = trace.as_deref_mut() {
        set_thread_sink(ctx.sink());
        set_thread_path(ctx.path_slot());
    }
    let out = maybe_trace(trace, "defender.expert.native_response", |trace| {
        let _ = trace;
        match eng.inspect(&ereq, map_level(level)) {
            Ok(Some(hit)) => Some(expert_hit_to_inspect_hit(
                InspectPhase::ResponseBody,
                &hit,
                &eng.pack_id(),
            )),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "nerve-antibody response inspect error");
                None
            }
        }
    });
    clear_thread_sink();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nerve_waf_core::WafAction;

    fn sample_hit(action: &str) -> ExpertHit {
        ExpertHit {
            total_score: 80,
            block_threshold: 50,
            action: action.into(),
            matched: vec![],
            rule_id: "x1".into(),
            rule_name: "X".into(),
            severity: "high".into(),
            evidence: "<script>".into(),
            param_path: Some("query.q".into()),
            decode_chain: Some(vec!["url_decode_uni".into()]),
            ..Default::default()
        }
    }

    #[test]
    fn maps_block_hit() {
        let hit = sample_hit("block");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Block);
        assert_eq!(ih.category, "expert");
        assert_eq!(ih.protection_stage.as_deref(), Some("expert"));
        assert_eq!(ih.rule_set.as_deref(), Some("expert-demo"));
        assert_eq!(ih.param_path.as_deref(), Some("query.q"));
        assert_eq!(
            ih.decode_chain,
            Some(vec!["url_decode_uni".into()])
        );
    }

    #[test]
    fn maps_challenge_hit_to_challenge_by_default() {
        let _guard = env_lock();
        std::env::remove_var("NERVE_EXPERT_CHALLENGE_AS");
        let hit = sample_hit("challenge");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Challenge);
    }

    #[test]
    fn maps_challenge_hit_env_override_log() {
        let _guard = env_lock();
        std::env::set_var("NERVE_EXPERT_CHALLENGE_AS", "log");
        let hit = sample_hit("challenge");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Log);
        std::env::remove_var("NERVE_EXPERT_CHALLENGE_AS");
    }

    #[test]
    fn maps_challenge_hit_env_override_block() {
        let _guard = env_lock();
        std::env::set_var("NERVE_EXPERT_CHALLENGE_AS", "block");
        let hit = sample_hit("challenge");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Block);
        std::env::remove_var("NERVE_EXPERT_CHALLENGE_AS");
    }

    #[test]
    fn maps_observe_and_unknown_to_log() {
        let hit = sample_hit("observe");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Log);
        let hit = sample_hit("unknown");
        let ih = expert_hit_to_inspect_hit(InspectPhase::RequestBody, &hit, "demo");
        assert_eq!(ih.action, WafAction::Log);
    }

    /// Serializes env-mutating tests (parallel cargo test).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn headers_map_parses_json_from_edge() {
        let json = r#"{"accept":"text/html","transfer-encoding":"chunked","host":"t.example"}"#;
        let m = headers_map(json);
        assert_eq!(m.get("transfer-encoding").map(String::as_str), Some("chunked"));
        assert_eq!(m.get("host").map(String::as_str), Some("t.example"));
        assert!(!m.contains_key("*"));
    }

    #[test]
    fn headers_map_parses_line_format() {
        let raw = "Host: t.example\nTransfer-Encoding: chunked\n";
        let m = headers_map(raw);
        assert_eq!(m.get("Host").map(String::as_str), Some("t.example"));
        assert_eq!(
            m.get("Transfer-Encoding").map(String::as_str),
            Some("chunked")
        );
    }

    #[test]
    fn loads_baseline_or_demo_pack() {
        if !expert_engine_enabled() {
            return;
        }
        let Some(eng) = ensure_engine() else {
            return;
        };
        assert!(
            eng.pack_id() == "baseline"
                || eng.pack_id() == "demo"
                || eng.pack_id() == "community-crs",
            "unexpected pack {}",
            eng.pack_id()
        );
        let req = InspectRequest {
            method: "GET".into(),
            host: "t".into(),
            uri: "/search?q=<script>alert(1)</script>".into(),
            headers: String::new(),
            body: String::new(),
            src_ip: "1.1.1.1".into(),
            user_agent: String::new(),
        };
        assert!(
            inspect_nerve_antibody(&req, SemLevel::Medium, &mut None).is_some(),
            "pack should block XSS sample"
        );
    }
}
