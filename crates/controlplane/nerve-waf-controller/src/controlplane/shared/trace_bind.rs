use nerve_waf_sdk::RequestTrace;
use serde_json::{json, Value};
use std::net::IpAddr;

pub fn is_ipv4_literal(host: &str) -> bool {
    host.parse::<IpAddr>()
        .map(|ip| ip.is_ipv4())
        .unwrap_or(false)
}

/// Build Trace Bind v2 body per docs/NERVE-PACKET-TRACE.md §5.
pub fn build_trace_bind_v2(
    trace: &RequestTrace,
    src_ip: &str,
    host: &str,
    port: u16,
    site_id: &str,
    pod: Option<&str>,
    namespace: Option<&str>,
) -> Value {
    let mut bind_keys = Vec::new();

    if let Some(sid) = trace
        .session_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(trace.journey_id.as_deref())
    {
        bind_keys.push(json!({
            "type": "session",
            "session_id": sid,
        }));
    }

    let host = host.trim();
    if !host.is_empty() && port > 0 {
        if is_ipv4_literal(host) {
            bind_keys.push(json!({
                "type": "tuple",
                "src_ip": src_ip,
                "dst_ip": host,
                "dst_port": port,
                "protocol": "tcp",
            }));
        } else {
            bind_keys.push(json!({
                "type": "host",
                "host": host,
                "dst_port": port,
            }));
        }
    }

    if let (Some(pod), Some(ns)) = (pod.filter(|s| !s.is_empty()), namespace.filter(|s| !s.is_empty()))
    {
        if is_ipv4_literal(host) && port > 0 {
            bind_keys.push(json!({
                "type": "pod_egress",
                "pod": pod,
                "namespace": ns,
                "dst_ip": host,
                "dst_port": port,
            }));
        }
    }

    let mut body = json!({
        "trace_id": trace.nerve_trace_id,
        "bind_scope": "ingress",
        "site_id": site_id,
        "bind_keys": bind_keys,
        "ttl_ms": 300_000,
    });

    if let Some(j) = &trace.journey_id {
        body["journey_id"] = json!(j);
    }
    if let Some(s) = trace
        .session_id
        .as_ref()
        .or(trace.journey_id.as_ref())
    {
        body["session_id"] = json!(s);
    }
    if let Some(p) = &trace.parent_journey_id {
        body["parent_journey_id"] = json!(p);
    }
    if !src_ip.is_empty() {
        body["src_ip"] = json!(src_ip);
    }
    if is_ipv4_literal(host) && port > 0 {
        body["dst_ip"] = json!(host);
        body["dst_port"] = json!(port);
        body["protocol"] = json!("tcp");
    } else if !host.is_empty() {
        body["dst_ip"] = json!(host);
        body["dst_port"] = json!(port);
    }

    body
}
