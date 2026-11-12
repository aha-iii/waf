use crate::{compile_rules, InspectRequest, L7Engine, RawChainStep, RawRule};
use nerve_waf_sdk::ProtectionModules;

#[test]
fn blocks_sqli_in_uri() {
    let engine = L7Engine::new(ProtectionModules::default());
    let hit = engine.inspect(&InspectRequest {
        uri: "/search?q=1+or+1%3D1".into(),
        ..Default::default()
    });
    assert!(hit.is_some());
    assert_eq!(hit.unwrap().category, "sqli");
}

#[test]
fn detect_mode_logs_only() {
    let mut modules = ProtectionModules::default();
    modules.mode = "detect".into();
    let engine = L7Engine::new(modules);
    let hit = engine
        .inspect(&InspectRequest {
            uri: "/x?q=<script>alert(1)</script>".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hit.action, "log");
    assert!(!hit.blocked);
}

#[test]
fn low_sensitivity_skips_medium_severity_rule() {
    let mut modules = ProtectionModules::default();
    modules.modules.get_mut("sqli").unwrap().sensitivity = "low".into();
    let rules = compile_rules(
        vec![RawRule {
            id: "t-low".into(),
            name: "test medium".into(),
            pattern: "MEDIUM_ONLY".into(),
            category: "sqli".into(),
            severity: "medium".into(),
            paranoia_level: 1,
            scan_targets: vec![],
            chain_steps: vec![],
        }],
        "test",
    )
    .unwrap();
    let engine = L7Engine::from_rules(modules, rules);
    assert!(engine
        .inspect(&InspectRequest {
            uri: "/?q=MEDIUM_ONLY".into(),
            ..Default::default()
        })
        .is_none());
}

#[test]
fn tenant_html_not_flagged_by_mongodb_sqli() {
    let modules = ProtectionModules::default();
    let rules = compile_rules(
        vec![RawRule {
            id: "942290".into(),
            name: "MongoDB SQLi".into(),
            pattern: r"(?i)\[?\$(?:where|gt|ne)\]?".into(),
            category: "sqli".into(),
            severity: "critical".into(),
            paranoia_level: 1,
            scan_targets: vec!["ARGS".into(), "COOKIES".into()],
            chain_steps: vec![],
        }],
        "test",
    )
    .unwrap();
    let engine = L7Engine::from_rules(modules, rules);
    let hit = engine.inspect(&InspectRequest {
        uri: "/tenant.html".into(),
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".into(),
        ..Default::default()
    });
    assert!(hit.is_none(), "expected no hit, got {:?}", hit);
}

#[test]
fn open_api_sign_not_flagged_as_rce_or_ssrf() {
    let mut modules = ProtectionModules::default();
    modules.modules.get_mut("rce").unwrap().enabled = true;
    modules.modules.get_mut("ssrf").unwrap().enabled = true;
    let engine = L7Engine::new(modules);
    let uri = "/open/api/ai/asr/speech/getInfo?tenant=cansino&appid=1665558862867&reqid=ad1715b0-815f-416c-893e-a3b25c6fe285&timestamp=1783299202393&platform=baidu&sign=7165D0A8A81529CDCA1FD9E811C5613EA387BF18352A2F5410B15CAF75524427";
    let hit = engine.inspect(&InspectRequest {
        uri: uri.into(),
        headers: "x-forwarded-for: 192.168.1.1:29002\nhost: app.example.com".into(),
        ..Default::default()
    });
    assert!(hit.is_none(), "expected no hit, got {:?}", hit);
}

#[test]
fn disabled_rule_skipped() {
    let mut modules = ProtectionModules::default();
    modules
        .modules
        .get_mut("sqli")
        .unwrap()
        .disabled_rules
        .push("t-dis".into());
    let rules = compile_rules(
        vec![RawRule {
            id: "t-dis".into(),
            name: "disabled".into(),
            pattern: "DISABLED_SIG".into(),
            category: "sqli".into(),
            severity: "high".into(),
            paranoia_level: 1,
            scan_targets: vec![],
            chain_steps: vec![],
        }],
        "test",
    )
    .unwrap();
    let engine = L7Engine::from_rules(modules, rules);
    assert!(engine
        .inspect(&InspectRequest {
            uri: "/?q=DISABLED_SIG".into(),
            ..Default::default()
        })
        .is_none());
}

#[test]
fn chain_requires_all_steps() {
    let rules = compile_rules(
        vec![RawRule {
            id: "chain-1".into(),
            name: "chain demo".into(),
            pattern: r"evil".into(),
            category: "rce".into(),
            severity: "high".into(),
            paranoia_level: 1,
            scan_targets: vec!["Referer".into()],
            chain_steps: vec![
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
        }],
        "test",
    )
    .unwrap();
    let engine = L7Engine::from_rules(ProtectionModules::default(), rules);

    assert!(engine
        .inspect(&InspectRequest {
            headers: "referer: http://x/evil".into(),
            ..Default::default()
        })
        .is_none());

    let hit = engine.inspect(&InspectRequest {
        headers: "referer: evil / path".into(),
        ..Default::default()
    });
    assert!(hit.is_some(), "expected chain hit");
    assert_eq!(hit.unwrap().rule_id, "chain-1");
}

#[test]
fn sql_comment_blocks_auth_bypass_tail() {
    let engine = L7Engine::new(ProtectionModules::default());
    let hit = engine.inspect(&InspectRequest {
        uri: "/login?user=admin'--".into(),
        ..Default::default()
    });
    assert!(hit.is_some(), "expected sql-003 hit");
    assert_eq!(hit.unwrap().rule_id, "sql-003");
}

#[test]
fn sql_comment_ignores_crm_server_type_dashes() {
    let engine = L7Engine::new(ProtectionModules::default());
    let body = r#"{"workers":[{"server_type":"道路机械--维修,star产品线--"}]}"#;
    let hit = engine.inspect(&InspectRequest {
        method: "POST".into(),
        uri: "/open/api/ai/dispatch/upload".into(),
        body: body.into(),
        ..Default::default()
    });
    assert!(
        hit.as_ref().map(|h| h.rule_id.as_str()) != Some("sql-003"),
        "CRM server_type dashes must not trigger sql-003, got {:?}",
        hit
    );
}

#[test]
fn dispatch_username_fullwidth_parens_not_encoded_evasion() {
    let engine = L7Engine::new(ProtectionModules::default());
    let uri = "/open/api/ai/dispatch/get_serverrole?tenantCode=demo&username=%E9%A9%AC%E5%AD%9F%E6%B5%A9%EF%BC%88%E5%86%85%E9%83%A8%E7%94%A8%E6%88%B7%EF%BC%89";
    let hit = engine.inspect(&InspectRequest {
        uri: uri.into(),
        ..Default::default()
    });
    assert!(
        hit.is_none(),
        "fullwidth parens in username must not be blocked by L7, got {:?}",
        hit
    );
}
