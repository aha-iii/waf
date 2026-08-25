//! Tests extracted from `lib.rs`.

use crate::*;

    #[test]
    fn whitelist_by_rule_and_uri() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "wl-1".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/health".into()],
                    rule_ids: vec!["sql-002".into()],
                    ..Default::default()
                },
            }],
        };
        let req = WhitelistRequest {
            host: "localhost",
            uri: "/api/health",
            src_ip: "1.2.3.4",
            rule_id: "sql-002",
            category: "sqli",
        };
        assert!(is_whitelisted(&cfg, &req));
    }

    #[test]
    fn inspect_bypass_by_uri_prefix() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "auth".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/auth-center/auth".into()],
                    ..Default::default()
                },
            }],
        };
        assert!(is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/auth-center/auth?sign=abc",
            "1.2.3.4"
        ));
        assert!(!is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/other",
            "1.2.3.4"
        ));
    }

    #[test]
    fn inspect_bypass_not_when_rule_id_scoped() {
        let cfg = WhitelistConfig {
            enabled: true,
            rules: vec![WhitelistRule {
                id: "partial".into(),
                enabled: true,
                comment: String::new(),
                match_on: WhitelistMatch {
                    uri_prefixes: vec!["/api/auth-center/auth".into()],
                    rule_ids: vec!["942370".into()],
                    ..Default::default()
                },
            }],
        };
        assert!(!is_inspect_bypassed(
            &cfg,
            "test.example.com",
            "/api/auth-center/auth",
            "1.2.3.4"
        ));
    }
