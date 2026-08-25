//! Behavioral anomaly scoring: IP sliding-window rate, path scanning, URI entropy heuristics.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SCAN_PATHS: &[&str] = &[
    "/.env",
    "/wp-admin",
    "/wp-login",
    "/phpmyadmin",
    "/admin",
    "/.git",
    "/actuator",
    "/api/v1",
    "/shell",
    "/console",
    "/manager",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    #[serde(default)]
    pub enabled: bool,
    /// block | log | challenge
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_threshold")]
    pub score_threshold: u8,
    /// Sliding window length in seconds.
    #[serde(default = "default_window")]
    pub window_sec: u64,
    /// Score up when request count in window exceeds this.
    #[serde(default = "default_rpm")]
    pub max_requests_per_window: u32,
    /// Score up when distinct URI count in window exceeds this.
    #[serde(default = "default_unique_paths")]
    pub max_unique_paths: u32,
}

fn default_action() -> String {
    "log".into()
}

fn default_threshold() -> u8 {
    70
}

fn default_window() -> u64 {
    60
}

fn default_rpm() -> u32 {
    120
}

fn default_unique_paths() -> u32 {
    40
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            action: default_action(),
            score_threshold: default_threshold(),
            window_sec: default_window(),
            max_requests_per_window: default_rpm(),
            max_unique_paths: default_unique_paths(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BehaviorVerdict {
    pub score: u8,
    pub is_anomaly: bool,
    pub reason: String,
    pub request_count: u32,
    pub unique_paths: u32,
}

struct IpWindow {
    started: Instant,
    request_count: u32,
    paths: HashSet<String>,
    methods: HashSet<String>,
}

pub struct BehaviorEngine {
    config: BehaviorConfig,
    windows: Mutex<HashMap<String, IpWindow>>,
}

impl BehaviorEngine {
    pub fn new(config: BehaviorConfig) -> Self {
        Self {
            config,
            windows: Mutex::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> &BehaviorConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: BehaviorConfig) {
        self.config = config;
    }

    pub fn score(&self, src_ip: &str, method: &str, uri: &str, user_agent: &str) -> BehaviorVerdict {
        score_with_state(
            &self.config,
            &mut self.windows.lock().unwrap_or_else(|e| e.into_inner()),
            src_ip,
            method,
            uri,
            user_agent,
        )
    }
}

pub fn score_with_config(
    src_ip: &str,
    method: &str,
    uri: &str,
    user_agent: &str,
    cfg: &BehaviorConfig,
) -> BehaviorVerdict {
    let mut map = HashMap::new();
    score_with_state(cfg, &mut map, src_ip, method, uri, user_agent)
}

fn score_with_state(
    cfg: &BehaviorConfig,
    windows: &mut HashMap<String, IpWindow>,
    src_ip: &str,
    method: &str,
    uri: &str,
    user_agent: &str,
) -> BehaviorVerdict {
    if !cfg.enabled {
        return BehaviorVerdict {
            score: 0,
            is_anomaly: false,
            reason: String::new(),
            request_count: 0,
            unique_paths: 0,
        };
    }

    let window = Duration::from_secs(cfg.window_sec.max(1));
    let path = uri.split('?').next().unwrap_or(uri).to_string();
    let now = Instant::now();

    let entry = windows.entry(src_ip.to_string()).or_insert_with(|| IpWindow {
        started: now,
        request_count: 0,
        paths: HashSet::new(),
        methods: HashSet::new(),
    });

    if now.duration_since(entry.started) > window {
        entry.started = now;
        entry.request_count = 0;
        entry.paths.clear();
        entry.methods.clear();
    }

    entry.request_count += 1;
    entry.paths.insert(path.clone());
    entry.methods.insert(method.to_ascii_uppercase());

    let mut score: u8 = 0;
    let mut reasons = Vec::new();

    if entry.request_count > cfg.max_requests_per_window {
        score = score.saturating_add(35);
        reasons.push("high_rpm");
    }

    let unique = entry.paths.len() as u32;
    if unique > cfg.max_unique_paths {
        score = score.saturating_add(30);
        reasons.push("path_scan");
    }

    let path_lower = path.to_lowercase();
    for probe in SCAN_PATHS {
        if path_lower.contains(probe) {
            score = score.saturating_add(15);
            reasons.push("probe_path");
            break;
        }
    }

    if uri_entropy(&path) > 4.2 {
        score = score.saturating_add(10);
        reasons.push("high_entropy_uri");
    }

    let m = method.to_ascii_uppercase();
    if m == "TRACE" || m == "TRACK" || (m == "OPTIONS" && entry.request_count > 20) {
        score = score.saturating_add(20);
        reasons.push("suspicious_method");
    }

    if user_agent.trim().is_empty() && entry.request_count > 5 {
        score = score.saturating_add(15);
        reasons.push("empty_ua_burst");
    }

    if entry.methods.len() > 6 {
        score = score.saturating_add(10);
        reasons.push("method_flood");
    }

    let is_anomaly = score >= cfg.score_threshold;
    BehaviorVerdict {
        score,
        is_anomaly,
        reason: reasons.join(","),
        request_count: entry.request_count,
        unique_paths: unique,
    }
}

fn uri_entropy(path: &str) -> f64 {
    if path.is_empty() {
        return 0.0;
    }
    let mut freq: HashMap<char, u32> = HashMap::new();
    for c in path.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }
    let len = path.len() as f64;
    freq.values()
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod test;
