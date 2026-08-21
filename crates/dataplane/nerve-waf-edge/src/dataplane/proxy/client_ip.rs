use axum::http::HeaderMap;
use std::net::SocketAddr;

/// Resolve client IP from proxy chain for inspect and attack logs.
///
/// Priority: leftmost `X-Forwarded-For` → `X-Real-IP` → TCP peer.
pub fn resolve_client_ip(headers: &HeaderMap, peer: &SocketAddr) -> String {
    if let Some(xff) = header_value(headers, "x-forwarded-for") {
        if let Some(ip) = first_forwarded_ip(&xff) {
            return ip;
        }
    }
    if let Some(xri) = header_value(headers, "x-real-ip") {
        if let Some(ip) = normalize_ip_token(xri.trim()) {
            return ip;
        }
    }
    peer.ip().to_string()
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn first_forwarded_ip(xff: &str) -> Option<String> {
    xff.split(',')
        .find_map(|part| normalize_ip_token(part.trim()))
}

fn normalize_ip_token(token: &str) -> Option<String> {
    let t = token.trim().trim_matches('"');
    if t.is_empty() {
        return None;
    }
    if t.starts_with('[') {
        let end = t.find(']')?;
        let inner = t[1..end].trim();
        if inner.is_empty() {
            return None;
        }
        return Some(inner.to_string());
    }
    // IPv4:port (single colon); pure IPv6 with many colons is not truncated here.
    if t.contains('.') && t.matches(':').count() == 1 {
        return Some(t.split(':').next()?.trim().to_string());
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

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
}
