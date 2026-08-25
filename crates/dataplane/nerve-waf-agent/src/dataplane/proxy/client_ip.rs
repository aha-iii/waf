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
