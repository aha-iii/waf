use serde_json::{json, Map, Value};
use uuid::Uuid;

pub const HEADER_NERVE_TRACE_ID: &str = "Nerve-Trace-Id";
pub const HEADER_X_NERVE_TRACE_ID: &str = "X-Nerve-Trace-Id";
pub const HEADER_NERVE_SPAN_ID: &str = "Nerve-Span-Id";
pub const HEADER_NERVE_JOURNEY_ID: &str = "Nerve-Journey-Id";
pub const HEADER_NERVE_PARENT_JOURNEY_ID: &str = "Nerve-Parent-Journey-Id";
pub const HEADER_NERVE_SESSION_ID: &str = "Nerve-Session-Id";
pub const HEADER_X_TRACE_ID: &str = "X-Trace-Id";
pub const HEADER_TRACEPARENT: &str = "traceparent";
pub const HEADER_TRACESTATE: &str = "tracestate";

#[derive(Debug, Clone, Default)]
pub struct NerveTraceState {
    pub journey_id: Option<String>,
    pub parent_journey_id: Option<String>,
    pub site_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RequestTrace {
    pub nerve_trace_id: String,
    pub nerve_span_id: String,
    pub x_trace_id: Option<String>,
    pub parent_nerve_span_id: Option<String>,
    pub journey_id: Option<String>,
    pub session_id: Option<String>,
    pub parent_journey_id: Option<String>,
    pub site_id: Option<String>,
}

impl RequestTrace {
    pub fn from_http_headers(get: impl Fn(&str) -> Option<String>) -> Self {
        let traceparent = first_header(&get, &[HEADER_TRACEPARENT, "Traceparent"]);
        let (tp_trace, tp_span) = traceparent
            .as_deref()
            .and_then(parse_traceparent)
            .unwrap_or_default();

        let tracestate = parse_tracestate(&first_header(&get, &[HEADER_TRACESTATE, "Tracestate"]).unwrap_or_default());

        let nerve_trace_id = first_header(
            &get,
            &[
                HEADER_NERVE_TRACE_ID,
                HEADER_X_NERVE_TRACE_ID,
                HEADER_X_TRACE_ID,
            ],
        )
        .or_else(|| (!tp_trace.is_empty()).then(|| tp_trace));
        let parent_nerve_span_id = first_header(&get, &[HEADER_NERVE_SPAN_ID])
            .or_else(|| (!tp_span.is_empty()).then(|| tp_span));
        let x_trace_id = first_header(&get, &[HEADER_X_TRACE_ID]);
        let journey_id = first_header(&get, &[HEADER_NERVE_JOURNEY_ID]).or(tracestate.journey_id);
        let session_id = first_header(&get, &[HEADER_NERVE_SESSION_ID]).or_else(|| journey_id.clone());
        let parent_journey_id =
            first_header(&get, &[HEADER_NERVE_PARENT_JOURNEY_ID]).or(tracestate.parent_journey_id);
        let site_id = tracestate.site_id;

        Self::new_waf(
            nerve_trace_id,
            parent_nerve_span_id,
            x_trace_id,
            journey_id,
            session_id,
            parent_journey_id,
            site_id,
        )
    }

    pub fn new_waf(
        nerve_trace_id: Option<String>,
        parent_nerve_span_id: Option<String>,
        x_trace_id: Option<String>,
        journey_id: Option<String>,
        session_id: Option<String>,
        parent_journey_id: Option<String>,
        site_id: Option<String>,
    ) -> Self {
        Self {
            nerve_trace_id: normalize_trace_id(
                nerve_trace_id
                    .or_else(|| x_trace_id.clone())
                    .as_deref(),
            ),
            nerve_span_id: normalize_span_id(None),
            x_trace_id,
            parent_nerve_span_id: parent_nerve_span_id
                .as_deref()
                .map(|s| normalize_span_id(Some(s))),
            journey_id,
            session_id,
            parent_journey_id,
            site_id,
        }
    }

    pub fn child_span(&self) -> Self {
        let mut next = self.clone();
        next.nerve_span_id = normalize_span_id(None);
        next
    }

    pub fn inject_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("nerve_trace_id".into(), json!(self.nerve_trace_id));
        obj.insert("nerve_span_id".into(), json!(self.nerve_span_id));
        if let Some(x) = &self.x_trace_id {
            obj.insert("x_trace_id".into(), json!(x));
        }
        if let Some(p) = &self.parent_nerve_span_id {
            obj.insert("parent_nerve_span_id".into(), json!(p));
        }
        if let Some(j) = &self.journey_id {
            obj.insert("journey_id".into(), json!(j));
        }
        if let Some(s) = &self.session_id {
            obj.insert("session_id".into(), json!(s));
        }
        if let Some(p) = &self.parent_journey_id {
            obj.insert("parent_journey_id".into(), json!(p));
        }
        if let Some(s) = &self.site_id {
            obj.insert("site_id".into(), json!(s));
        }
    }

    pub fn merge(&self, mut rec: Value) -> Value {
        if let Some(obj) = rec.as_object_mut() {
            self.inject_json(obj);
        }
        rec
    }

    pub fn outbound_http_headers(&self) -> Vec<(&'static str, String)> {
        let mut out = vec![
            (HEADER_NERVE_TRACE_ID, self.nerve_trace_id.clone()),
            (HEADER_NERVE_SPAN_ID, self.nerve_span_id.clone()),
        ];
        if let Some(j) = &self.journey_id {
            out.push((HEADER_NERVE_JOURNEY_ID, j.clone()));
        }
        if let Some(s) = &self.session_id {
            out.push((HEADER_NERVE_SESSION_ID, s.clone()));
        }
        if let Some(p) = &self.parent_journey_id {
            out.push((HEADER_NERVE_PARENT_JOURNEY_ID, p.clone()));
        }
        if let Some(x) = &self.x_trace_id {
            out.push((HEADER_X_TRACE_ID, x.clone()));
        }
        out.push((HEADER_TRACEPARENT, self.w3c_traceparent()));
        if let Some(ts) = self.w3c_tracestate() {
            out.push((HEADER_TRACESTATE, ts));
        }
        out
    }

    pub fn w3c_traceparent(&self) -> String {
        format!(
            "00-{}-{}-01",
            normalize_w3c_id(&self.nerve_trace_id, 32),
            normalize_w3c_id(&self.nerve_span_id, 16)
        )
    }

    pub fn w3c_tracestate(&self) -> Option<String> {
        format_tracestate(
            self.journey_id.as_deref(),
            self.parent_journey_id.as_deref(),
            self.site_id.as_deref(),
        )
    }
}

/// W3C traceparent: `version-trace_id-span_id-flags`
pub fn parse_traceparent(raw: &str) -> Option<(String, String)> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 || parts[0] != "00" {
        return None;
    }
    let trace_id = parts[1].trim();
    let span_id = parts[2].trim();
    if trace_id.len() != 32 || span_id.len() != 16 {
        return None;
    }
    if !trace_id.chars().all(is_hex) || !span_id.chars().all(is_hex) {
        return None;
    }
    if trace_id.chars().all(|c| c == '0') {
        return None;
    }
    Some((
        trace_id.to_ascii_lowercase(),
        span_id.to_ascii_lowercase(),
    ))
}

/// Parse `tracestate` nerve vendor segment: `nerve=journey:…,parent:…,site:…`
pub fn parse_tracestate(raw: &str) -> NerveTraceState {
    let mut out = NerveTraceState::default();
    for part in raw.split(',') {
        let part = part.trim();
        let inner = part
            .strip_prefix("nerve=")
            .or_else(|| part.strip_prefix("Nerve="));
        let Some(inner) = inner else {
            continue;
        };
        for kv in inner.split(',') {
            let Some((k, v)) = kv.split_once(':') else {
                continue;
            };
            match k.trim() {
                "journey" => out.journey_id = Some(v.trim().to_string()),
                "parent" => out.parent_journey_id = Some(v.trim().to_string()),
                "site" => out.site_id = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    out
}

pub fn format_tracestate(
    journey_id: Option<&str>,
    parent_journey_id: Option<&str>,
    site_id: Option<&str>,
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(j) = journey_id.filter(|s| !s.is_empty()) {
        parts.push(format!("journey:{j}"));
    }
    if let Some(p) = parent_journey_id.filter(|s| !s.is_empty()) {
        parts.push(format!("parent:{p}"));
    }
    if let Some(s) = site_id.filter(|s| !s.is_empty()) {
        parts.push(format!("site:{s}"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("nerve={}", parts.join(",")))
}

pub fn normalize_trace_id(raw: Option<&str>) -> String {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => new_trace_id_hex(),
        Some(s) if is_hex_len(s, 32) => s.to_ascii_lowercase(),
        Some(s) => {
            let hex: String = s
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect::<String>()
                .to_ascii_lowercase();
            if hex.len() >= 32 {
                hex[..32].to_string()
            } else {
                stable_hex32(s)
            }
        }
    }
}

pub fn normalize_span_id(raw: Option<&str>) -> String {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => new_span_id_hex(),
        Some(s) if is_hex_len(s, 16) => s.to_ascii_lowercase(),
        Some(s) => {
            let hex: String = s
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect::<String>()
                .to_ascii_lowercase();
            if hex.len() >= 16 {
                hex[..16].to_string()
            } else {
                stable_hex32(&format!("span|{s}"))[..16].to_string()
            }
        }
    }
}

fn stable_hex32(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    input.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    format!("{input}#nerve-trace").hash(&mut h2);
    format!("{:016x}{:016x}", h1.finish(), h2.finish())
}

fn bytes_to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn new_trace_id_hex() -> String {
    bytes_to_hex(Uuid::new_v4().as_bytes())
}

fn new_span_id_hex() -> String {
    bytes_to_hex(&Uuid::new_v4().as_bytes()[..8])
}

fn first_header(get: &impl Fn(&str) -> Option<String>, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(v) = get(name) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn normalize_w3c_id(raw: &str, width: usize) -> String {
    let hex: String = raw
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();
    if hex.len() >= width {
        return hex[..width].to_string();
    }
    format!("{:0>width$}", hex, width = width)
}

fn is_hex_len(s: &str, n: usize) -> bool {
    s.len() == n && s.chars().all(is_hex)
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
