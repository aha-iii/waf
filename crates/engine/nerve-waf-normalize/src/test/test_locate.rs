//! Tests extracted from `locate/locate.rs`.

use crate::locate::*;
use nerve_waf_l7::InspectRequest;

    #[test]
    fn locates_query_and_json() {
        let req = InspectRequest {
            uri: "/api?id=1&name=test".into(),
            body: r#"{"msg":"hello","nested":{"x":"1 union select"}}"#.into(),
            ..Default::default()
        };
        let slices = locate_params(&req);
        assert!(slices.iter().any(|s| s.path == "query.id"));
        assert!(slices.iter().any(|s| s.raw.contains("union select")));
    }

    #[test]
    fn scope_skips_headers() {
        let req = InspectRequest {
            uri: "/?id=1".into(),
            headers: "X-Test: evil\n".into(),
            ..Default::default()
        };
        let scope = DetectionScope {
            header: false,
            ..Default::default()
        };
        let slices = locate_params_scoped(&req, &scope);
        assert!(slices.iter().any(|s| s.path == "query.id"));
        assert!(!slices.iter().any(|s| s.path.starts_with("header.")));
    }
