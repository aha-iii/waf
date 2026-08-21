//! Service-level allowlist policy helpers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAllowPolicy {
    pub id: String,
    pub namespace: String,
    pub service: String,
    pub allow_destinations: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}

pub fn validate_policy(p: &ServiceAllowPolicy) -> Result<(), String> {
    if p.namespace.is_empty() || p.service.is_empty() {
        return Err("namespace and service required".into());
    }
    Ok(())
}

pub fn policies_for_pod<'a>(
    policies: &'a [ServiceAllowPolicy],
    namespace: &str,
    service: &str,
) -> Vec<&'a ServiceAllowPolicy> {
    policies
        .iter()
        .filter(|p| p.enabled && p.namespace == namespace && p.service == service)
        .collect()
}
