//! Tests extracted from `trace/trace.rs`.

use crate::trace::*;

    #[test]
    fn parse_w3c_traceparent() {
        let (trace, span) = parse_traceparent(
            "00-0e650cf06eac9365e740a790f36ea9b2-2227d07fe0c328c9-00",
        )
        .unwrap();
        assert_eq!(trace, "0e650cf06eac9365e740a790f36ea9b2");
        assert_eq!(span, "2227d07fe0c328c9");
    }

    #[test]
    fn parse_tracestate_nerve() {
        let ts = parse_tracestate("vendor1=abc,nerve=journey:j1,parent:p0,site:demo");
        assert_eq!(ts.journey_id.as_deref(), Some("j1"));
        assert_eq!(ts.parent_journey_id.as_deref(), Some("p0"));
        assert_eq!(ts.site_id.as_deref(), Some("demo"));
    }

    #[test]
    fn normalize_uuid_to_hex32() {
        let id = normalize_trace_id(Some("550e8400-e29b-41d4-a716-446655440000"));
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
