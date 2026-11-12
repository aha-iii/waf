//! Tests extracted from `expert_bridge/expert_bridge.rs`.

use crate::expert_bridge::*;
use nerve_antibody::ExpertHit;
use nerve_waf_core::{InspectPhase, WafAction};
use nerve_waf_l7::InspectRequest;
use nerve_waf_semantic::ExpertLevel as SemLevel;

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
    assert_eq!(ih.decode_chain, Some(vec!["url_decode_uni".into()]));
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
    assert_eq!(
        m.get("transfer-encoding").map(String::as_str),
        Some("chunked")
    );
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
        eng.pack_id() == "baseline" || eng.pack_id() == "demo" || eng.pack_id() == "community-crs",
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
