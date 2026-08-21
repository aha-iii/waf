use nerve_waf_l7::InspectRequest;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Query,
    BodyForm,
    BodyJson,
    Header,
    Cookie,
    Path,
    Raw,
}

/// Detect scope: query / body / header / cookie / path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetectionScope {
    #[serde(default = "default_true")]
    pub query: bool,
    #[serde(default = "default_true")]
    pub body: bool,
    #[serde(default = "default_true")]
    pub header: bool,
    #[serde(default = "default_true")]
    pub cookie: bool,
    #[serde(default = "default_true")]
    pub path: bool,
}

impl Default for DetectionScope {
    fn default() -> Self {
        Self {
            query: true,
            body: true,
            header: true,
            cookie: true,
            path: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSlice {
    pub path: String,
    pub raw: String,
    pub kind: ParamKind,
}

pub fn locate_params(req: &InspectRequest) -> Vec<ParamSlice> {
    locate_params_scoped(req, &DetectionScope::default())
}

pub fn locate_params_scoped(req: &InspectRequest, scope: &DetectionScope) -> Vec<ParamSlice> {
    let mut out = Vec::new();

    if scope.path && !req.uri.is_empty() {
        let path = req.uri.split('?').next().unwrap_or(&req.uri);
        if !path.is_empty() && path != "/" {
            out.push(ParamSlice {
                path: "path".into(),
                raw: path.to_string(),
                kind: ParamKind::Path,
            });
        }
    }

    if scope.query && !req.uri.is_empty() {
        for (name, value) in parse_query_pairs(&req.uri) {
            if !value.is_empty() {
                out.push(ParamSlice {
                    path: format!("query.{name}"),
                    raw: value,
                    kind: ParamKind::Query,
                });
            }
            if !name.is_empty() {
                out.push(ParamSlice {
                    path: format!("query.{name}@name"),
                    raw: name,
                    kind: ParamKind::Query,
                });
            }
        }
    }

    if scope.body && !req.body.is_empty() {
        if req.body.trim_start().starts_with('{') || req.body.trim_start().starts_with('[') {
            collect_json_strings("$", &req.body, &mut out);
        } else if !req.body.starts_with('{') {
            for (name, value) in parse_query_pairs(&format!("?{}", req.body)) {
                if !value.is_empty() {
                    out.push(ParamSlice {
                        path: format!("body.{name}"),
                        raw: value,
                        kind: ParamKind::BodyForm,
                    });
                }
            }
            if out.iter().all(|p| p.kind != ParamKind::BodyForm) {
                out.push(ParamSlice {
                    path: "body".into(),
                    raw: req.body.clone(),
                    kind: ParamKind::Raw,
                });
            }
        }
    }

    if scope.header {
        for (name, value) in parse_header_values(&req.headers) {
            if !value.is_empty() {
                out.push(ParamSlice {
                    path: format!("header.{name}"),
                    raw: value,
                    kind: ParamKind::Header,
                });
            }
        }
        if !req.user_agent.is_empty() {
            out.push(ParamSlice {
                path: "header.user-agent".into(),
                raw: req.user_agent.clone(),
                kind: ParamKind::Header,
            });
        }
    }

    if scope.cookie {
        for (name, value) in parse_cookies(&req.headers) {
            if !value.is_empty() {
                out.push(ParamSlice {
                    path: format!("cookie.{name}"),
                    raw: value,
                    kind: ParamKind::Cookie,
                });
            }
        }
    }

    out
}

fn parse_query_pairs(uri: &str) -> Vec<(String, String)> {
    let q = uri.split('?').nth(1).unwrap_or(uri.trim_start_matches('?'));
    let mut pairs = Vec::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => pairs.push((k.to_string(), v.to_string())),
            None => pairs.push((pair.to_string(), String::new())),
        }
    }
    pairs
}

fn parse_header_values(headers: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in headers.lines() {
        let Some((name, val)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name == "cookie" {
            continue;
        }
        let val = val.trim();
        if !val.is_empty() {
            out.push((name, val.to_string()));
        }
    }
    out
}

fn parse_cookies(headers: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in headers.lines() {
        if !line.to_ascii_lowercase().starts_with("cookie:") {
            continue;
        }
        let rest = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
        for part in rest.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.split_once('=') {
                Some((k, v)) => out.push((k.trim().into(), v.trim().into())),
                None => out.push((part.into(), String::new())),
            }
        }
    }
    out
}

fn collect_json_strings(prefix: &str, json: &str, out: &mut Vec<ParamSlice>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        out.push(ParamSlice {
            path: prefix.into(),
            raw: json.to_string(),
            kind: ParamKind::BodyJson,
        });
        return;
    };
    walk_json(prefix, &v, out);
}

fn walk_json(prefix: &str, v: &serde_json::Value, out: &mut Vec<ParamSlice>) {
    match v {
        serde_json::Value::String(s) if !s.is_empty() => {
            out.push(ParamSlice {
                path: prefix.into(),
                raw: s.clone(),
                kind: ParamKind::BodyJson,
            });
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                walk_json(&format!("{prefix}[{i}]"), item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, item) in map {
                walk_json(&format!("{prefix}.{k}"), item, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
