use crate::scan_targets::ScanTarget;
use crate::{compile_rules, embedded_rules, RawChainStep, RawRule};

fn raw(
    id: &str,
    pattern: &str,
    category: &str,
    severity: &str,
    targets: &[&str],
    chain: Vec<RawChainStep>,
) -> RawRule {
    RawRule {
        id: id.into(),
        name: id.into(),
        pattern: pattern.into(),
        category: category.into(),
        severity: severity.into(),
        paranoia_level: 1,
        scan_targets: targets.iter().map(|s| (*s).into()).collect(),
        chain_steps: chain,
    }
}

#[test]
fn embedded_rules_non_empty() {
    assert!(!embedded_rules().is_empty());
}

#[test]
fn compiles_chain_rule() {
    let rules = compile_rules(
        vec![raw(
            "932206",
            r"\$",
            "rce",
            "high",
            &["Referer"],
            vec![
                RawChainStep {
                    pattern: "/".into(),
                    scan_targets: vec![],
                    on_matched: true,
                },
                RawChainStep {
                    pattern: r"\s".into(),
                    scan_targets: vec![],
                    on_matched: true,
                },
            ],
        )],
        "test",
    )
    .unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].steps.len(), 3);
    assert!(rules[0].steps[1].on_matched);
}

#[test]
fn ssrf_rule_uses_args_only_targets() {
    let rules = embedded_rules();
    let ssrf = rules.iter().find(|r| r.id == "ssrf-001").unwrap();
    assert_eq!(
        ssrf.scan_targets,
        vec![
            ScanTarget::Args,
            ScanTarget::RequestFilename,
            ScanTarget::Referer,
        ]
    );
}
