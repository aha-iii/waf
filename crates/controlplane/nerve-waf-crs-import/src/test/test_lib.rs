//! Tests extracted from `lib.rs`.

use crate::*;
use nerve_waf_l7::compile_rules;

    #[test]
    fn parses_simple_rx_rule() {
        let chunk = r#"SecRule ARGS "@rx (?i)select\b" "id:942100,phase:2,block,msg:'SQLi',tag:'attack-sqli',tag:'paranoia-level/1',severity:'CRITICAL'"#;
        let rule = parse_secrule(chunk, 4).unwrap();
        assert_eq!(rule.id, "942100");
        assert_eq!(rule.category, "sqli");
        assert!(rule.pattern.contains("select"));
        assert!(compile_rules(vec![rule.clone()], "test").is_ok());
    }

    #[test]
    fn parses_crs_modsecurity_quoted_rx() {
        let chunk = r#"SecRule REQUEST_COOKIES|ARGS "@rx (?i)\bunion\b[\s\x0b]+select\b" "id:942270,phase:2,block,msg:'SQLi union',tag:'attack-sqli',tag:'paranoia-level/1',severity:'CRITICAL'"#;
        let rule = parse_secrule(chunk, 2).unwrap();
        assert_eq!(rule.id, "942270");
        assert!(compile_rules(vec![rule], "test").is_ok());
    }

    #[test]
    fn skips_detectsqli() {
        let chunk = r#"SecRule ARGS "@detectSQLi" "id:942100,phase:2,block,msg:'libinjection'"#;
        assert!(parse_secrule(chunk, 4).is_none());
    }

    #[test]
    fn lone_chain_parent_skipped_by_parse_secrule() {
        let chunk = r#"SecRule REQUEST_HEADERS:Referer "@rx test" "id:932206,phase:1,block,msg:'RCE Bypass Technique',tag:'paranoia-level/2',severity:'CRITICAL',chain""#;
        assert!(parse_secrule(chunk, 4).is_none());
    }

    #[test]
    fn parses_chain_group_with_matched_vars() {
        let group = vec![
            r#"SecRule REQUEST_HEADERS:Referer "@rx evil" "id:932206,phase:1,block,msg:'RCE Bypass',tag:'paranoia-level/2',severity:'CRITICAL',chain""#.into(),
            r#"SecRule MATCHED_VARS "@rx /" "chain""#.into(),
            r#"SecRule MATCHED_VARS "@rx \s" "t:none""#.into(),
        ];
        let rule = parse_chain_group(&group, 4).unwrap();
        assert_eq!(rule.id, "932206");
        assert_eq!(rule.chain_steps.len(), 2);
        assert!(rule.chain_steps[0].on_matched);
        assert!(rule.chain_steps[1].on_matched);
        assert!(compile_rules(vec![rule], "test").is_ok());
    }

    #[test]
    fn parses_referer_scan_target() {
        let chunk = r#"SecRule REQUEST_HEADERS:Referer "@rx evil" "id:999001,phase:1,block,msg:'test',tag:'paranoia-level/1',severity:'HIGH'"#;
        let rule = parse_secrule(chunk, 4).unwrap();
        assert_eq!(rule.scan_targets, vec!["Referer"]);
    }

    #[test]
    fn parses_args_pipe_targets() {
        let chunk = r#"SecRule REQUEST_COOKIES|ARGS "@rx union" "id:942270,phase:2,block,msg:'SQLi',tag:'attack-sqli',tag:'paranoia-level/1',severity:'CRITICAL'"#;
        let rule = parse_secrule(chunk, 4).unwrap();
        assert!(rule.scan_targets.contains(&"ARGS".to_string()));
        assert!(rule.scan_targets.contains(&"COOKIES".to_string()));
        assert_eq!(rule.paranoia_level, 1);
    }

    #[test]
    fn preserves_mongodb_regex_escapes() {
        let chunk = r#"SecRule ARGS "@rx (?i)\[?\$(?:abs|where)\]?" "id:942290,phase:2,block,msg:'Mongo',tag:'attack-sqli',tag:'paranoia-level/1',severity:'CRITICAL'"#;
        let rule = parse_secrule(chunk, 4).unwrap();
        assert!(rule.pattern.contains(r"\[?\$"), "pattern={}", rule.pattern);
        assert!(compile_rules(vec![rule.clone()], "test").is_ok());
    }
