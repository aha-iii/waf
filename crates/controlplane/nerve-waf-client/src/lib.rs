//! HTTP client to Nerve Agent / Observer (policy sync, journey traces).

use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

static WAF_SYNC_REVISION: AtomicU64 = AtomicU64::new(1);
static CONTROL_SYNC_REVISION: AtomicU64 = AtomicU64::new(1);

const SYNCPLAN_API_VERSION: &str = "syncplan.nerve.io/v1";

#[derive(Debug, Error)]
pub enum NerveClientError {
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("{0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Clone)]
pub struct NerveClient {
    base_url: String,
    http: Client,
}

impl NerveClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { base_url, http }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("NERVE_URL").unwrap_or_else(|_| "http://127.0.0.1:8766".into());
        Self::new(url)
    }

    pub async fn deny_ip(&self, ip: &str) -> Result<(), NerveClientError> {
        self.post_query("/api/policy/deny", &[("ip", ip)]).await
    }

    pub async fn allow_ip(&self, ip: &str) -> Result<(), NerveClientError> {
        self.post_query("/api/policy/allow", &[("ip", ip)]).await
    }

    pub async fn baseline_allow(&self, pod: &str, ip: &str) -> Result<(), NerveClientError> {
        self.post_query("/api/baseline/allow", &[("pod", pod), ("ip", ip)])
            .await
    }

    pub async fn baseline_enforce(&self, pod: Option<&str>) -> Result<(), NerveClientError> {
        match pod {
            Some(p) if !p.is_empty() => {
                self.post_query("/api/baseline/enforce", &[("pod", p)]).await
            }
            _ => self.post_empty("/api/baseline/enforce").await,
        }
    }

    /// Apply SyncPlan. By default wraps **WAF-L3** envelope (`source=waf`).
    /// Set `NERVE_SYNC_LEGACY=1` to send the bare Agent body (compat window).
    pub async fn sync_plan(&self, body: &Value) -> Result<Value, NerveClientError> {
        if sync_legacy_enabled() {
            return self.post_sync_raw(body).await;
        }
        let (payload, scopes) = waf_payload_and_scopes(body);
        let revision = WAF_SYNC_REVISION.fetch_add(1, Ordering::Relaxed);
        let envelope = wrap_sync_envelope("waf", revision, &scopes, &payload);
        self.post_sync_raw(&envelope).await
    }

    /// Cluster / control-plane SyncPlan (`source=control`): NP L4 / identity / control deny.
    pub async fn sync_plan_control(&self, body: &Value) -> Result<Value, NerveClientError> {
        if sync_legacy_enabled() {
            return self.post_sync_raw(body).await;
        }
        let scopes = control_scopes_for(body);
        let revision = CONTROL_SYNC_REVISION.fetch_add(1, Ordering::Relaxed);
        let envelope = wrap_sync_envelope("control", revision, &scopes, body);
        self.post_sync_raw(&envelope).await
    }

    async fn post_sync_raw(&self, body: &Value) -> Result<Value, NerveClientError> {
        let url = format!("{}/api/sync/plan", self.base_url);
        let res = self.http.post(&url).json(body).send().await?;
        let status = res.status();
        let text = res.text().await?;
        if !status.is_success() {
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body: text,
            });
        }
        Ok(serde_json::from_str(&text).unwrap_or(json!({"status": "ok"})))
    }

    pub async fn post_empty(&self, path: &str) -> Result<(), NerveClientError> {
        let url = format!("{}{}", self.base_url, path);
        let res = self.http.post(&url).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }

    pub async fn get_status(&self) -> Result<Value, NerveClientError> {
        self.get("/api/observer/status").await
    }

    pub async fn get_policy_status(&self) -> Result<Value, NerveClientError> {
        self.get("/api/policy/status").await
    }

    pub async fn get_blacklist(&self) -> Result<Value, NerveClientError> {
        self.get("/api/policy/blacklist").await
    }

    pub async fn get_audit_export(&self, limit: u32) -> Result<Value, NerveClientError> {
        let limit_s = limit.to_string();
        self.get_query("/api/audit/export", &[("limit", &limit_s)])
            .await
    }

    pub async fn get_sessions(&self, limit: u32) -> Result<Value, NerveClientError> {
        let limit_s = limit.to_string();
        self.get_query("/api/observer/sessions", &[("limit", &limit_s), ("open_limit", &limit_s)])
            .await
    }

    pub async fn get_anomalies(&self, limit: u32) -> Result<Value, NerveClientError> {
        let limit_s = limit.to_string();
        self.get_query("/api/observer/anomalies", &[("limit", &limit_s)])
            .await
    }

    pub async fn get_policy_hits(&self, limit: u32) -> Result<Value, NerveClientError> {
        let limit_s = limit.to_string();
        self.get_query("/api/policy/hits", &[("limit", &limit_s)])
            .await
    }

    pub async fn get_journeys(
        &self,
        limit: u32,
        trace_id: Option<&str>,
        signal_only: Option<bool>,
        since_minutes: Option<u32>,
    ) -> Result<Value, NerveClientError> {
        let limit_s = limit.to_string();
        let mut params: Vec<(&str, &str)> = vec![("limit", &limit_s)];
        let trace_buf;
        if let Some(tid) = trace_id.filter(|s| !s.trim().is_empty()) {
            trace_buf = tid.to_string();
            params.push(("trace_id", &trace_buf));
        }
        let signal_buf;
        if let Some(v) = signal_only {
            signal_buf = if v { "1" } else { "0" }.to_string();
            params.push(("signal_only", &signal_buf));
        }
        let since_buf;
        if let Some(m) = since_minutes.filter(|m| *m > 0) {
            since_buf = m.to_string();
            params.push(("since_minutes", &since_buf));
        }
        self.get_query("/api/observer/journeys", &params).await
    }

    pub async fn get_journey(&self, id: &str) -> Result<Value, NerveClientError> {
        self.get_query("/api/observer/journey", &[("id", id)]).await
    }

    /// NPT connection lineage (journey tree under trace_id + merged timeline).
    pub async fn get_trace_genealogy(&self, trace_id: &str) -> Result<Value, NerveClientError> {
        self.get_query("/api/observer/trace/genealogy", &[("trace_id", trace_id)])
            .await
    }

    /// Bind trace context to Agent session/5-tuple for eBPF/CNI auto-stitch.
    pub async fn bind_trace_context(&self, body: &Value) -> Result<(), NerveClientError> {
        let url = format!("{}/api/observer/trace/bind", self.base_url);
        let res = self.http.post(&url).json(body).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }

    pub async fn post_journey_hops(&self, hops: &[Value]) -> Result<(), NerveClientError> {
        if hops.is_empty() {
            return Ok(());
        }
        let url = format!("{}/api/observer/journey/hops", self.base_url);
        let body = serde_json::json!({ "hops": hops });
        let res = self.http.post(&url).json(&body).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }

    /// Report one hop to Observer (Packet Journey standard).
    pub async fn post_journey_hop(&self, hop: &Value) -> Result<(), NerveClientError> {
        let url = format!("{}/api/observer/journey/hop", self.base_url);
        let res = self.http.post(&url).json(hop).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }

    /// Agent live observation snapshot (same as WebSocket initial state).
    pub async fn get_observer_state(&self) -> Result<Value, NerveClientError> {
        self.get("/api/state").await
    }

    pub async fn get_session_graph(&self, session_id: &str) -> Result<Value, NerveClientError> {
        self.get_query("/api/observer/session", &[("id", session_id)])
            .await
    }

    pub async fn get_baseline(&self) -> Result<Value, NerveClientError> {
        self.get("/api/baseline").await
    }

    pub async fn explain_anomaly(&self, index: u32) -> Result<Value, NerveClientError> {
        let idx = index.to_string();
        self.get_query("/api/anomalies/explain", &[("index", &idx)])
            .await
    }

    async fn get_query(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, NerveClientError> {
        let url = format!("{}{}", self.base_url, path);
        let res = self.http.get(&url).query(query).send().await?;
        let status = res.status();
        let body = res.text().await?;
        if !status.is_success() {
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(serde_json::from_str(&body).unwrap_or(Value::String(body)))
    }

    pub async fn get(&self, path: &str) -> Result<Value, NerveClientError> {
        let url = format!("{}{}", self.base_url, path);
        let res = self.http.get(&url).send().await?;
        let status = res.status();
        let body = res.text().await?;
        if !status.is_success() {
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(serde_json::from_str(&body).unwrap_or(Value::String(body)))
    }

    async fn post_query(&self, path: &str, query: &[(&str, &str)]) -> Result<(), NerveClientError> {
        let url = format!("{}{}", self.base_url, path);
        let res = self.http.post(&url).query(query).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(NerveClientError::Http {
                status: status.as_u16(),
                body,
            });
        }
        Ok(())
    }
}

fn sync_legacy_enabled() -> bool {
    matches!(
        std::env::var("NERVE_SYNC_LEGACY").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

/// Build SyncPlan v1 envelope (see contracts/syncplan/v1/openapi.yaml).
pub fn wrap_sync_envelope(
    source: &str,
    revision: u64,
    replace_scopes: &[String],
    payload: &Value,
) -> Value {
    json!({
        "api_version": SYNCPLAN_API_VERSION,
        "source": source,
        "revision": revision.max(1),
        "idempotency_key": Uuid::new_v4().to_string(),
        "replace_scopes": replace_scopes,
        "generated_at": chrono_like_now(),
        "payload": payload,
    })
}

fn chrono_like_now() -> String {
    // Avoid extra chrono dep in client: RFC3339-ish UTC from system time.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Strip control-only fields from a WAF sync body and infer replace_scopes.
pub fn waf_payload_and_scopes(body: &Value) -> (Value, Vec<String>) {
    let mut payload = body.clone();
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("l4_policy");
        obj.remove("identity");
        obj.remove("fqdn_policy");
    }
    let mut scopes = Vec::new();
    let has_deny = payload
        .get("deny")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
        || payload
            .get("deny_ips")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    let has_allow = payload
        .get("allow_ips")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_baseline = payload
        .get("baseline")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if has_deny {
        scopes.push("deny.waf".into());
    }
    if has_allow {
        scopes.push("allow.waf".into());
    }
    if has_baseline {
        scopes.push("baseline".into());
    }
    if payload.get("l7_mode").is_some() {
        scopes.push("l7_mode".into());
    }
    if scopes.is_empty() {
        scopes.push("deny.waf".into());
    }
    (payload, scopes)
}

fn control_scopes_for(body: &Value) -> Vec<String> {
    let mut scopes = Vec::new();
    let has_deny = body
        .get("deny")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
        || body
            .get("deny_ips")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    if has_deny {
        scopes.push("deny.control".into());
    }
    if body.get("l4_policy").is_some() {
        scopes.push("l4_policy".into());
    }
    if body.get("identity").is_some() {
        scopes.push("identity".into());
    }
    if body.get("fqdn_policy").is_some() {
        scopes.push("fqdn_policy".into());
    }
    if scopes.is_empty() {
        scopes.push("l4_policy".into());
    }
    scopes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waf_envelope_strips_l4() {
        let body = json!({
            "deny": ["1.1.1.1"],
            "l4_policy": {"version": 1, "rules": []},
            "l7_mode": "block"
        });
        let (payload, scopes) = waf_payload_and_scopes(&body);
        assert!(payload.get("l4_policy").is_none());
        assert!(scopes.contains(&"deny.waf".into()));
        assert!(scopes.contains(&"l7_mode".into()));
        let env = wrap_sync_envelope("waf", 3, &scopes, &payload);
        assert_eq!(env["api_version"], SYNCPLAN_API_VERSION);
        assert_eq!(env["source"], "waf");
        assert_eq!(env["revision"], 3);
    }
}
