//! Tests extracted from `lib.rs`.

use crate::*;
use serde_json::json;

    #[test]
    fn waf_envelope_strips_l4() {
        let body = json!({
            "deny": ["1.1.1.1"],
            "l4_policy": {"version": 1, "rules": []},
            "l7_mode": "block"
        });
        let (payload, scopes) = waf_payload_and_scopes(&body);
        assert!(payload.get("l4_policy").is_none());
        assert!(scopes.contains(&"deny.waf".into()));
        assert!(scopes.contains(&"l7_mode".into()));
        let env = wrap_sync_envelope("waf", 3, &scopes, &payload);
        assert_eq!(env["api_version"], SYNCPLAN_API_VERSION);
        assert_eq!(env["source"], "waf");
        assert_eq!(env["revision"], 3);
    }
