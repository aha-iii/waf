//! Response-body integrity / anti-tamper baselines.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperPage {
    pub id: String,
    pub host: String,
    pub path: String,
    pub baseline_hash: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TamperConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub pages: Vec<TamperPage>,
}

pub struct TamperGuard {
    cfg: RwLock<TamperConfig>,
    learned: RwLock<HashMap<String, String>>,
}

impl TamperGuard {
    pub fn new(cfg: TamperConfig) -> Self {
        Self {
            cfg: RwLock::new(cfg),
            learned: RwLock::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> TamperConfig {
        self.cfg.read().unwrap().clone()
    }

    pub fn set_config(&self, cfg: TamperConfig) {
        *self.cfg.write().unwrap() = cfg;
    }

    pub fn hash_body(body: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(body);
        format!("{:x}", h.finalize())
    }

    pub fn check(&self, host: &str, path: &str, body: &[u8]) -> Option<TamperAlert> {
        let cfg = self.cfg.read().unwrap();
        if !cfg.enabled {
            return None;
        }
        let page = cfg.pages.iter().find(|p| {
            p.enabled && p.host == host && (p.path == path || p.path == "/*")
        })?;
        let actual = Self::hash_body(body);
        if actual != page.baseline_hash {
            return Some(TamperAlert {
                page_id: page.id.clone(),
                host: host.into(),
                path: path.into(),
                expected: page.baseline_hash.clone(),
                actual,
            });
        }
        None
    }

    pub fn learn_baseline(&self, host: &str, path: &str, body: &[u8]) -> String {
        let h = Self::hash_body(body);
        self.learned
            .write()
            .unwrap()
            .insert(format!("{host}|{path}"), h.clone());
        h
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TamperAlert {
    pub page_id: String,
    pub host: String,
    pub path: String,
    pub expected: String,
    pub actual: String,
}
