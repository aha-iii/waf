use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

const CONFIG_NAMES: &[&str] = &["config.yml", "config.yaml"];

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
    NotFound,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "read config: {e}"),
            Self::Parse(e) => write!(f, "parse config: {e}"),
            Self::NotFound => write!(
                f,
                "config file not found (set WAF_CONFIG or place config.yml in project root)"
            ),
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_yaml::Error> for ConfigError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::Parse(e)
    }
}

/// 根配置：`waf:` + 顶层 `logging:`
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WafFileConfig {
    pub waf: WafNodeConfig,
    #[serde(default)]
    pub logging: WafLoggingConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WafNodeConfig {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: String,
    #[serde(default = "default_nerve_url")]
    pub nerve_url: String,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub console: ConsoleConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ApiConfig {
    #[serde(default = "default_api_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct DatabaseConfig {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WafLoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_access_log")]
    pub access_log: String,
    #[serde(default = "default_audit_log")]
    pub audit_log: String,
    #[serde(default = "default_error_log")]
    pub error_log: String,
    #[serde(default = "default_server_log")]
    pub server_log: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_log_rotate")]
    pub rotate: String,
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    #[serde(default = "default_max_backups")]
    pub max_backups: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ConsoleConfig {
    #[serde(default = "default_console_port")]
    pub port: u16,
}

fn default_api_port() -> u16 {
    8780
}

fn default_console_port() -> u16 {
    5174
}

fn default_data_dir() -> String {
    "./data".into()
}

fn default_plugins_dir() -> String {
    "./plugins".into()
}

fn default_nerve_url() -> String {
    "http://127.0.0.1:8766".into()
}

fn default_log_level() -> String {
    "info".into()
}

fn default_access_log() -> String {
    "./logs/access.log".into()
}

fn default_audit_log() -> String {
    "./logs/audit.log".into()
}

fn default_error_log() -> String {
    "./logs/error.log".into()
}

fn default_server_log() -> String {
    "./logs/server.log".into()
}

fn default_log_format() -> String {
    "json".into()
}

fn default_log_rotate() -> String {
    "daily".into()
}

fn default_max_size_mb() -> u64 {
    100
}

fn default_max_age_days() -> u32 {
    7
}

fn default_max_backups() -> u32 {
    14
}

impl Default for WafFileConfig {
    fn default() -> Self {
        Self {
            waf: WafNodeConfig::default(),
            logging: WafLoggingConfig::default(),
        }
    }
}

impl Default for WafNodeConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig {
                port: default_api_port(),
            },
            data_dir: default_data_dir(),
            plugins_dir: default_plugins_dir(),
            nerve_url: default_nerve_url(),
            database: DatabaseConfig::default(),
            console: ConsoleConfig {
                port: default_console_port(),
            },
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            port: default_api_port(),
        }
    }
}

impl Default for WafLoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            access_log: default_access_log(),
            audit_log: default_audit_log(),
            error_log: default_error_log(),
            server_log: default_server_log(),
            format: default_log_format(),
            rotate: default_log_rotate(),
            max_size_mb: default_max_size_mb(),
            max_age_days: default_max_age_days(),
            max_backups: default_max_backups(),
        }
    }
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            port: default_console_port(),
        }
    }
}

impl WafFileConfig {
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let path = resolve_config_path(path)?;
        let raw = std::fs::read_to_string(&path)?;
        let parsed: Self = serde_yaml::from_str(&raw)?;
        Ok(parsed)
    }

    pub fn load_or_default(path: Option<&Path>) -> Self {
        match Self::load(path) {
            Ok(file) => file,
            Err(ConfigError::NotFound) => Self::default(),
            Err(_) => Self::default(),
        }
    }
}

pub fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Some(p) = path {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(ConfigError::NotFound);
    }
    if let Ok(p) = std::env::var("WAF_CONFIG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Some(p) = find_config_upward() {
        return Ok(p);
    }
    Err(ConfigError::NotFound)
}

fn find_config_upward() -> Option<PathBuf> {
    if let Ok(mut dir) = std::env::current_dir() {
        for _ in 0..8 {
            for name in CONFIG_NAMES {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            if !dir.pop() {
                break;
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(mut dir) = exe.parent().map(|p| p.to_path_buf()) {
            for _ in 0..6 {
                for name in CONFIG_NAMES {
                    let candidate = dir.join(name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }
    None
}
