//! Tests extracted from `dataplane/proxy/client_ip/client_ip.rs`.

use crate::dataplane::proxy::client_ip::*;
use axum::http::{HeaderMap, HeaderValue};
use std::net::SocketAddr;

    fn peer() -> SocketAddr {
        "10.0.0.1:54321".parse().unwrap()
    }

    #[test]
    fn xff_leftmost_wins() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.5, 192.168.1.10, 10.0.0.1"),
        );
        assert_eq!(resolve_client_ip(&h, &peer()), "203.0.113.5");
    }

    #[test]
    fn xff_strips_port() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.168.1.1:29002"),
        );
        assert_eq!(resolve_client_ip(&h, &peer()), "192.168.1.1");
    }

    #[test]
    fn x_real_ip_fallback() {
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", HeaderValue::from_static("198.51.100.2"));
        assert_eq!(resolve_client_ip(&h, &peer()), "198.51.100.2");
    }

    #[test]
    fn peer_fallback_when_no_headers() {
        let h = HeaderMap::new();
        assert_eq!(resolve_client_ip(&h, &peer()), "10.0.0.1");
    }
