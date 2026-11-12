//! Tests extracted from `config/config.rs`.

use crate::config::*;

#[test]
fn expand_legacy_crs_to_expert_and_core() {
    let order = normalize_pipeline_order(&["semantic".into(), "crs".into(), "custom".into()]);
    assert_eq!(order, vec!["pre_access", "expert", "core", "custom"]);
    let enabled =
        normalize_pipeline_enabled(&order, &[("crs".into(), false)].into_iter().collect());
    assert_eq!(enabled.get("expert"), Some(&false));
    assert_eq!(enabled.get("core"), Some(&false));
    assert!(!enabled.contains_key("semantic"));
}

#[test]
fn migrate_semantic_stage_off_disables_enhancement() {
    let mut cfg = SemanticConfig::default();
    cfg.engines
        .pipeline_enabled
        .insert("semantic".into(), false);
    cfg.engines.pipeline_order = vec![
        "semantic".into(),
        "expert".into(),
        "core".into(),
        "custom".into(),
    ];
    let cfg = cfg.normalized();
    assert!(!cfg.enabled);
    assert_eq!(
        cfg.engines.pipeline_order,
        vec!["pre_access", "expert", "core", "custom"]
    );
}
