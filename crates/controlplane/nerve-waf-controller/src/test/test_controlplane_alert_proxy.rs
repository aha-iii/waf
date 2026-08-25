//! Tests extracted from `controlplane/alert_proxy/alert_proxy.rs`.

use crate::controlplane::alert_proxy::*;

    #[test]
    fn build_upstream_preserves_path_and_query() {
        let u = build_upstream_uri(
            "http://127.0.0.1:8790/",
            "/api/alerts/config?product=waf",
        )
        .unwrap();
        assert_eq!(u.path(), "/api/alerts/config");
        assert_eq!(u.query(), Some("product=waf"));
        assert_eq!(u.scheme_str(), Some("http"));
    }

    #[test]
    fn build_upstream_acks_nested_path() {
        let u = build_upstream_uri(
            "http://nerve-alert:8790",
            "/api/alerts/logs/abc/ack",
        )
        .unwrap();
        assert_eq!(u.path(), "/api/alerts/logs/abc/ack");
    }

    #[test]
    fn build_upstream_rejects_empty_base() {
        assert!(build_upstream_uri("", "/api/alerts/config").is_err());
    }
