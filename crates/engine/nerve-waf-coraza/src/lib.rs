//! Bridge to process-embedded Coraza + OWASP CRS (`nerve-waf-coraza` Go binary).
//!
//! Enable with `NERVE_WAF_CORAZA_URL=http://127.0.0.1:8791` or `NERVE_WAF_CRS_ENGINE=coraza`.

use nerve_waf_l7::{InspectRequest, InspectResult};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum CorazaError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("coraza disabled")]
    Disabled,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize)]
struct CorazaInspectBody {
    method: String,
    host: String,
    uri: String,
    headers: std::collections::HashMap<String, String>,
    body: String,
    src_ip: String,
    user_agent: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CorazaInspectResp {
    #[serde(default)]
    blocked: bool,
    #[serde(default)]
    action: String,
    #[serde(default)]
    rule_id: Option<String>,
    #[serde(default)]
    rule_name: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    engine: Option<String>,
}

/// Resolved base URL when Coraza CRS engine is enabled.
pub fn coraza_url_from_env() -> Option<String> {
    if let Ok(u) = std::env::var("NERVE_WAF_CORAZA_URL") {
        let u = u.trim().trim_end_matches('/').to_string();
        if !u.is_empty() {
            return Some(u);
        }
    }
    match std::env::var("NERVE_WAF_CRS_ENGINE") {
        Ok(v) if matches!(v.trim().to_ascii_lowercase().as_str(), "coraza" | "crs-coraza" | "full") => {
            Some(
                std::env::var("NERVE_CORAZA_LISTEN")
                    .ok()
                    .map(|a| {
                        let a = a.trim().to_string();
                        if a.starts_with("http://") || a.starts_with("https://") {
                            a
                        } else {
                            format!("http://{a}")
                        }
                    })
                    .unwrap_or_else(|| "http://127.0.0.1:8791".into()),
            )
        }
        _ => None,
    }
}

pub fn coraza_enabled() -> bool {
    coraza_url_from_env().is_some()
}

fn http_client() -> &'static reqwest::blocking::Client {
    static C: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(
                std::env::var("NERVE_WAF_CORAZA_TIMEOUT_MS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(800),
            ))
            .build()
            .expect("coraza http client")
    })
}

fn parse_headers_map(headers: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let t = headers.trim();
    if t.is_empty() {
        return map;
    }
    if t.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
            if let Some(obj) = v.as_object() {
                for (k, val) in obj {
                    let s = val
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| val.to_string());
                    map.insert(k.clone(), s);
                }
                return map;
            }
        }
    }
    for line in t.lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

/// Blocking inspect (for embed / sync pipeline paths).
pub fn inspect_blocking(req: &InspectRequest) -> Result<Option<InspectResult>, CorazaError> {
    let Some(base) = coraza_url_from_env() else {
        return Err(CorazaError::Disabled);
    };
    let body = CorazaInspectBody {
        method: req.method.clone(),
        host: req.host.clone(),
        uri: req.uri.clone(),
        headers: parse_headers_map(&req.headers),
        body: req.body.clone(),
        src_ip: req.src_ip.clone(),
        user_agent: req.user_agent.clone(),
    };
    let url = format!("{base}/api/inspect");
    let resp = http_client()
        .post(&url)
        .json(&body)
        .send()
        .map_err(CorazaError::Http)?;
    if !resp.status().is_success() {
        return Err(CorazaError::Other(format!(
            "coraza HTTP {}",
            resp.status()
        )));
    }
    let v: CorazaInspectResp = resp.json().map_err(CorazaError::Http)?;
    if !v.blocked && v.action != "block" && v.action != "deny" {
        return Ok(None);
    }
    Ok(Some(InspectResult {
        blocked: true,
        action: if v.action.is_empty() {
            "block".into()
        } else {
            v.action
        },
        category: "crs".into(),
        rule_id: v.rule_id.unwrap_or_else(|| "coraza".into()),
        rule_name: v
            .rule_name
            .or(v.message)
            .unwrap_or_else(|| "专家防护".into()),
        severity: v.severity.unwrap_or_else(|| "critical".into()),
        matched_field: "request".into(),
        payload_sample: truncate(&req.uri, 160),
        plugin_name: Some("coraza".into()),
        plugin_type: Some(v.engine.unwrap_or_else(|| "coraza+crs".into())),
        param_path: None,
        decode_chain: None,
        semantic_score: None,
        engines: Some(vec!["coraza".into()]),
    }))
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_without_env() {
        std::env::remove_var("NERVE_WAF_CORAZA_URL");
        std::env::remove_var("NERVE_WAF_CRS_ENGINE");
        assert!(!coraza_enabled());
    }
}
