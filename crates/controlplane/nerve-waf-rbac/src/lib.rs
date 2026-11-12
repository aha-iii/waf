//! Multi-tenant RBAC helpers.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RbacConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tenants: Vec<Tenant>,
    /// role -> permissions e.g. "admin" -> ["*:*"]
    #[serde(default)]
    pub role_permissions: HashMap<String, Vec<String>>,
    /// api_key -> (tenant_id, role)
    #[serde(default)]
    pub api_keys: HashMap<String, ApiKeyBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyBinding {
    pub tenant_id: String,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub tenant_id: String,
    pub role: String,
}

pub fn resolve_key(cfg: &RbacConfig, key: &str) -> Option<AuthContext> {
    if !cfg.enabled {
        return Some(AuthContext {
            tenant_id: "default".into(),
            role: "admin".into(),
        });
    }
    cfg.api_keys.get(key).map(|b| AuthContext {
        tenant_id: b.tenant_id.clone(),
        role: b.role.clone(),
    })
}

pub fn check_permission(cfg: &RbacConfig, role: &str, resource: &str, action: &str) -> bool {
    if !cfg.enabled {
        return true;
    }
    let perms = match cfg.role_permissions.get(role) {
        Some(p) => p,
        None => return false,
    };
    let need = format!("{resource}:{action}");
    let wild_all = "*:*".to_string();
    let wild_res = format!("{resource}:*");
    perms
        .iter()
        .any(|p| p == &wild_all || p == &wild_res || p == &need)
}

pub fn default_config() -> RbacConfig {
    let mut role_permissions = HashMap::new();
    role_permissions.insert("admin".into(), vec!["*:*".into()]);
    role_permissions.insert(
        "operator".into(),
        vec!["policy:*".into(), "topology:read".into()],
    );
    role_permissions.insert(
        "viewer".into(),
        vec!["topology:read".into(), "events:read".into()],
    );
    RbacConfig {
        enabled: false,
        tenants: vec![Tenant {
            id: "default".into(),
            name: "Default".into(),
            enabled: true,
        }],
        role_permissions,
        api_keys: HashMap::new(),
    }
}

pub fn merge_env_keys(cfg: &mut RbacConfig) {
    if let Ok(k) = std::env::var("NERVE_API_KEY") {
        cfg.api_keys.insert(
            k,
            ApiKeyBinding {
                tenant_id: "default".into(),
                role: "admin".into(),
            },
        );
    }
}

pub fn list_permissions(cfg: &RbacConfig, role: &str) -> HashSet<String> {
    cfg.role_permissions
        .get(role)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default()
}
