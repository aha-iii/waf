//! Nerve-alert integration: HTTP dispatch when `alert` feature is on; no-op stubs otherwise.

#[cfg(feature = "alert")]
mod imp {
    use chrono::{DateTime, Utc};
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    pub mod product {
        pub const WAF: &str = "waf";
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AlertPayload {
        #[serde(default = "default_product")]
        pub product: String,
        #[serde(default = "default_tenant")]
        pub tenant: String,
        pub ts: DateTime<Utc>,
        pub severity: String,
        pub category: String,
        pub action: String,
        pub src_ip: Option<String>,
        pub host: Option<String>,
        pub uri: Option<String>,
        pub message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub site_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub span_id: Option<String>,
    }

    fn default_product() -> String {
        product::WAF.into()
    }

    fn default_tenant() -> String {
        "default".into()
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DispatchRequest {
        #[serde(default = "default_product")]
        pub product: String,
        #[serde(default = "default_tenant")]
        pub tenant: String,
        pub trigger: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub event_id: Option<String>,
        #[serde(default)]
        pub test: bool,
        pub event: AlertPayload,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SourceStatsOverview {
        pub total_events: u64,
        pub blocked: u64,
        pub logged: u64,
        #[serde(default)]
        pub by_category: Vec<CategoryCount>,
        #[serde(default)]
        pub total_requests: u64,
        #[serde(default)]
        pub clean_passed: u64,
        #[serde(default)]
        pub observed_passed: u64,
        #[serde(default)]
        pub blocked_requests: u64,
        #[serde(default)]
        pub block_rate: f64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CategoryCount {
        pub category: String,
        pub count: u64,
    }

    #[derive(Clone)]
    pub struct AlertClient {
        base_url: String,
        product: String,
        tenant: String,
        enabled: bool,
        http: Client,
    }

    impl AlertClient {
        pub fn from_env(product: impl Into<String>) -> Self {
            let base_url = std::env::var("NERVE_ALERT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8790".into())
                .trim_end_matches('/')
                .to_string();
            let enabled = std::env::var("NERVE_ALERT_ENABLED")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(true);
            let http = Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("reqwest client");
            Self {
                base_url,
                product: product.into(),
                tenant: "default".into(),
                enabled,
                http,
            }
        }

        pub fn spawn_dispatch(&self, mut req: DispatchRequest) {
            if !self.enabled {
                return;
            }
            if req.product.is_empty() {
                req.product = self.product.clone();
            }
            if req.tenant.is_empty() {
                req.tenant = self.tenant.clone();
            }
            let url = format!("{}/api/v1/events/dispatch", self.base_url);
            let http = self.http.clone();
            tokio::spawn(async move {
                match http.post(&url).json(&req).send().await {
                    Ok(res) if res.status().is_success() => {}
                    Ok(res) => {
                        let status = res.status().as_u16();
                        let body = res.text().await.unwrap_or_default();
                        tracing::warn!(status, body = %body, "nerve-alert dispatch failed");
                    }
                    Err(e) => tracing::warn!(error = %e, "nerve-alert dispatch failed"),
                }
            });
        }
    }

    pub fn new_alert_client() -> AlertClient {
        AlertClient::from_env(product::WAF)
    }
}

#[cfg(not(feature = "alert"))]
mod imp {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    pub mod product {
        pub const WAF: &str = "waf";
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AlertPayload {
        #[serde(default = "default_product")]
        pub product: String,
        #[serde(default = "default_tenant")]
        pub tenant: String,
        pub ts: DateTime<Utc>,
        pub severity: String,
        pub category: String,
        pub action: String,
        pub src_ip: Option<String>,
        pub host: Option<String>,
        pub uri: Option<String>,
        pub message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub site_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trace_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub span_id: Option<String>,
    }

    fn default_product() -> String {
        product::WAF.into()
    }

    fn default_tenant() -> String {
        "default".into()
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DispatchRequest {
        #[serde(default = "default_product")]
        pub product: String,
        #[serde(default = "default_tenant")]
        pub tenant: String,
        pub trigger: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub event_id: Option<String>,
        #[serde(default)]
        pub test: bool,
        pub event: AlertPayload,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SourceStatsOverview {
        pub total_events: u64,
        pub blocked: u64,
        pub logged: u64,
        #[serde(default)]
        pub by_category: Vec<CategoryCount>,
        #[serde(default)]
        pub total_requests: u64,
        #[serde(default)]
        pub clean_passed: u64,
        #[serde(default)]
        pub observed_passed: u64,
        #[serde(default)]
        pub blocked_requests: u64,
        #[serde(default)]
        pub block_rate: f64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CategoryCount {
        pub category: String,
        pub count: u64,
    }

    #[derive(Clone)]
    pub struct AlertClient;

    impl AlertClient {
        pub fn from_env(_product: impl Into<String>) -> Self {
            Self
        }

        pub fn spawn_dispatch(&self, req: DispatchRequest) {
            tracing::debug!(
                event_id = ?req.event_id,
                trigger = %req.trigger,
                "alert dispatch skipped (build with --features alert and set NERVE_ALERT_URL)"
            );
        }
    }

    pub fn new_alert_client() -> AlertClient {
        AlertClient::from_env(product::WAF)
    }
}

pub use imp::*;
