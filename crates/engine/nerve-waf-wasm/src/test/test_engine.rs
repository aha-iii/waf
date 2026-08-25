//! Tests extracted from `engine/engine.rs`.

use crate::engine::*;
use crate::resolve_wasm_path;
use nerve_waf_l7::InspectRequest;

    #[test]
    fn disabled_when_engine_off() {
        std::env::set_var("NERVE_WAF_CRS_ENGINE", "off");
        assert!(!wasm_crs_enabled());
        std::env::remove_var("NERVE_WAF_CRS_ENGINE");
    }

    #[test]
    fn blocks_obvious_sqli_when_wasm_present() {
        std::env::set_var("NERVE_WAF_CRS_ENGINE", "wasm");
        let path = resolve_wasm_path();
        if path.is_none() {
            eprintln!("skip: coraza wasm not on disk");
            return;
        }
        let req = InspectRequest {
            method: "POST".into(),
            host: "localhost".into(),
            uri: "/api/login".into(),
            headers: "Content-Type: application/x-www-form-urlencoded".into(),
            body: "1' ORDER BY 3--+".into(),
            src_ip: "10.0.0.1".into(),
            user_agent: "nerve-waf-test".into(),
        };
        let hit = inspect_blocking(&req).expect("inspect");
        assert!(hit.is_some(), "expected CRS block for SQLi payload");
        let hit = hit.unwrap();
        assert!(hit.blocked);
        std::env::remove_var("NERVE_WAF_CRS_ENGINE");
    }
