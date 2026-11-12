//! API security: OpenAPI path/schema baselines, auth-endpoint abuse, sensitive body fields.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiSecConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub block_unknown_paths: bool,
    #[serde(default)]
    pub schemas: Vec<ApiSchema>,
    /// Auth-related path prefixes (e.g. /api/login) for rate limiting.
    #[serde(default)]
    pub auth_paths: Vec<String>,
    #[serde(default = "default_auth_rpm")]
    pub auth_requests_per_minute: u32,
    /// Sensitive body-field regex (e.g. password, token).
    #[serde(default = "default_sensitive_patterns")]
    pub sensitive_patterns: Vec<String>,
    #[serde(default = "default_true")]
    pub scan_request_body: bool,
}

fn default_true() -> bool {
    true
}

fn default_auth_rpm() -> u32 {
    30
}

fn default_sensitive_patterns() -> Vec<String> {
    vec![
        r"(?i)(password\s*[:=]|passwd\s*[:=])".into(),
        r"(?i)(api[_-]?key\s*[:=]|secret\s*[:=]|authorization\s*:\s*bearer)".into(),
        r"(?i)(-----BEGIN (RSA |EC )?PRIVATE KEY-----)".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSchema {
    pub id: String,
    pub host: String,
    #[serde(default)]
    pub paths: HashMap<String, PathSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSpec {
    pub methods: Vec<String>,
    #[serde(default)]
    pub required_query: Vec<String>,
    #[serde(default)]
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiSecViolation {
    pub schema_id: String,
    pub reason: String,
    pub path: String,
    pub method: String,
}

static AUTH_BUCKETS: std::sync::OnceLock<RwLock<HashMap<String, (f64, Instant)>>> =
    std::sync::OnceLock::new();

fn auth_buckets() -> &'static RwLock<HashMap<String, (f64, Instant)>> {
    AUTH_BUCKETS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn validate_request(
    cfg: &ApiSecConfig,
    host: &str,
    method: &str,
    path: &str,
    query: &str,
    body_len: usize,
    body: &str,
) -> Option<ApiSecViolation> {
    if !cfg.enabled {
        return None;
    }

    if let Some(v) = check_auth_abuse(cfg, host, path, method) {
        return Some(v);
    }

    if cfg.scan_request_body && !body.is_empty() {
        if let Some(v) = check_sensitive_body(cfg, host, path, method, body) {
            return Some(v);
        }
    }

    let schema = cfg
        .schemas
        .iter()
        .find(|s| s.host == host || s.host == "*")?;
    let path_spec = schema.paths.get(path);
    if path_spec.is_none() {
        if cfg.block_unknown_paths {
            return Some(ApiSecViolation {
                schema_id: schema.id.clone(),
                reason: "unknown_path".into(),
                path: path.into(),
                method: method.into(),
            });
        }
        return None;
    }
    let spec = path_spec.unwrap();
    let method_up = method.to_uppercase();
    if !spec
        .methods
        .iter()
        .any(|m| m.eq_ignore_ascii_case(&method_up))
    {
        return Some(ApiSecViolation {
            schema_id: schema.id.clone(),
            reason: "method_not_allowed".into(),
            path: path.into(),
            method: method.into(),
        });
    }
    for req in &spec.required_query {
        if !query.contains(req) {
            return Some(ApiSecViolation {
                schema_id: schema.id.clone(),
                reason: format!("missing_query:{req}"),
                path: path.into(),
                method: method.into(),
            });
        }
    }
    if spec.max_body_bytes > 0 && body_len > spec.max_body_bytes {
        return Some(ApiSecViolation {
            schema_id: schema.id.clone(),
            reason: "body_too_large".into(),
            path: path.into(),
            method: method.into(),
        });
    }
    None
}

fn check_auth_abuse(
    cfg: &ApiSecConfig,
    host: &str,
    path: &str,
    method: &str,
) -> Option<ApiSecViolation> {
    if cfg.auth_paths.is_empty() {
        return None;
    }
    let is_auth = cfg
        .auth_paths
        .iter()
        .any(|p| path.starts_with(p) || path.contains(p));
    if !is_auth {
        return None;
    }
    let limit = cfg.auth_requests_per_minute.max(1) as f64;
    let key = format!("{host}|{path}|{}", method.to_uppercase());
    let now = Instant::now();
    let mut map = auth_buckets().write().ok()?;
    let entry = map.entry(key.clone()).or_insert((limit, now));
    let elapsed = now.duration_since(entry.1).as_secs_f64();
    entry.0 = (entry.0 + elapsed * (limit / 60.0)).min(limit);
    entry.1 = now;
    if entry.0 < 1.0 {
        return Some(ApiSecViolation {
            schema_id: "auth-abuse".into(),
            reason: "auth_rate_limit".into(),
            path: path.into(),
            method: method.into(),
        });
    }
    entry.0 -= 1.0;
    None
}

fn check_sensitive_body(
    cfg: &ApiSecConfig,
    host: &str,
    path: &str,
    method: &str,
    body: &str,
) -> Option<ApiSecViolation> {
    for pat in &cfg.sensitive_patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if re.is_match(body) {
                return Some(ApiSecViolation {
                    schema_id: host.into(),
                    reason: "sensitive_field_in_request".into(),
                    path: path.into(),
                    method: method.into(),
                });
            }
        }
    }
    None
}
