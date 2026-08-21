//! Bot detection and scoring.

use serde::{Deserialize, Serialize};

const BOT_UA: &[&str] = &[
    "curl", "wget", "python-requests", "scrapy", "bot", "spider", "crawler",
    "headless", "phantomjs", "selenium",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// block | log | challenge
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_threshold")]
    pub score_threshold: u8,
}

fn default_true() -> bool {
    true
}

fn default_action() -> String {
    "log".into()
}

fn default_threshold() -> u8 {
    50
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            action: default_action(),
            score_threshold: default_threshold(),
        }
    }
}

pub struct BotVerdict {
    pub score: u8,
    pub is_bot: bool,
    pub reason: String,
}

pub fn score(user_agent: &str, uri: &str) -> BotVerdict {
    score_with_config(user_agent, uri, &BotConfig::default())
}

pub fn score_with_config(user_agent: &str, uri: &str, cfg: &BotConfig) -> BotVerdict {
    let ua = user_agent.to_lowercase();
    let mut score: u8 = 0;
    let mut reasons = Vec::new();

    for sig in BOT_UA {
        if ua.contains(sig) {
            score = score.saturating_add(40);
            reasons.push(*sig);
        }
    }
    if uri.contains("robots.txt") {
        score = score.saturating_add(20);
        reasons.push("robots.txt");
    }
    if ua.is_empty() {
        score = score.saturating_add(30);
        reasons.push("empty_ua");
    }
    let is_bot = score >= cfg.score_threshold;
    BotVerdict {
        score,
        is_bot,
        reason: reasons.join(","),
    }
}
