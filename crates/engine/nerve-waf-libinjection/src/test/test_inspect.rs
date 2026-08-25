//! Tests extracted from `inspect/inspect.rs`.

use crate::inspect::*;
use crate::LibInjectionConfig;
use nerve_waf_l7::InspectRequest;
use nerve_waf_sdk::ProtectionModules;

    #[test]
    fn detects_sqli_in_query() {
        let cfg = LibInjectionConfig::default();
        let modules = ProtectionModules::default();
        let hit = inspect_libinjection(
            &cfg,
            &modules,
            &InspectRequest {
                uri: "/?id=1' OR '1'='1".into(),
                ..Default::default()
            },
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().rule_id, "942100");
    }
