//! Threat-intelligence feed helpers.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelEntry {
    pub ip: String,
    pub category: String,
    pub score: u8,
}

pub struct IntelFeed {
    entries: HashSet<String>,
    categories: HashMap<String, String>,
}

impl IntelFeed {
    pub fn from_entries(list: Vec<IntelEntry>) -> Self {
        let mut entries = HashSet::new();
        let mut categories = HashMap::new();
        for e in list {
            entries.insert(e.ip.clone());
            categories.insert(e.ip, e.category);
        }
        Self { entries, categories }
    }

    pub fn is_malicious(&self, ip: &str) -> bool {
        self.entries.contains(ip)
    }

    pub fn category(&self, ip: &str) -> Option<&str> {
        self.categories.get(ip).map(|s| s.as_str())
    }
}

impl Default for IntelFeed {
    fn default() -> Self {
        Self::from_entries(vec![])
    }
}
