//! GeoIP (MaxMind MMDB + CIDR fallback) with country allow/block lists.

use ipnet::IpNet;
use maxminddb::{geoip2, MaxMindDbError, Reader};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub block_countries: Vec<String>,
    #[serde(default)]
    pub allow_countries: Vec<String>,
    /// Path to GeoLite2-Country.mmdb; also via GEOIP_DB_PATH env.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mmdb_path: Option<String>,
    /// mmdb | cidr | auto (prefer MMDB).
    #[serde(default = "default_geo_source")]
    pub source: String,
}

fn default_geo_source() -> String {
    "auto".into()
}

impl Default for GeoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            block_countries: Vec::new(),
            allow_countries: Vec::new(),
            mmdb_path: None,
            source: default_geo_source(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoCidrRule {
    pub cidr: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLookupResult {
    pub ip: String,
    pub country: Option<String>,
    pub allowed: bool,
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeoStatus {
    pub mmdb_loaded: bool,
    pub mmdb_path: Option<String>,
    pub cidr_rules: usize,
    pub source: String,
}

pub struct GeoEngine {
    config: GeoConfig,
    rules: Vec<(IpNet, String)>,
    mmdb: Option<Arc<Reader<Vec<u8>>>>,
    mmdb_path: Option<PathBuf>,
}

impl GeoEngine {
    pub fn new(config: GeoConfig, cidrs: Vec<GeoCidrRule>) -> Self {
        let mut engine = Self {
            config: config.clone(),
            rules: Vec::new(),
            mmdb: None,
            mmdb_path: None,
        };
        engine.reload_cidrs(cidrs);
        let _ = engine.try_load_mmdb(resolve_mmdb_path(&config));
        engine
    }

    pub fn with_builtin(config: GeoConfig) -> Self {
        Self::new(config, builtin_cidrs())
    }

    pub fn update_config(&mut self, config: GeoConfig) {
        let path_changed = config.mmdb_path != self.config.mmdb_path
            || config.source != self.config.source;
        self.config = config;
        if path_changed {
            let _ = self.try_load_mmdb(resolve_mmdb_path(&self.config));
        }
    }

    pub fn config(&self) -> &GeoConfig {
        &self.config
    }

    pub fn status(&self) -> GeoStatus {
        GeoStatus {
            mmdb_loaded: self.mmdb.is_some(),
            mmdb_path: self
                .mmdb_path
                .as_ref()
                .map(|p| p.to_string_lossy().into()),
            cidr_rules: self.rules.len(),
            source: self.config.source.clone(),
        }
    }

    pub fn reload_cidrs(&mut self, cidrs: Vec<GeoCidrRule>) {
        self.rules.clear();
        for r in cidrs {
            if let Ok(net) = IpNet::from_str(&r.cidr) {
                self.rules.push((net, r.country.to_uppercase()));
            }
        }
    }

    pub fn reload_mmdb(&mut self) -> Result<bool, String> {
        self.mmdb = None;
        self.mmdb_path = None;
        self.try_load_mmdb(resolve_mmdb_path(&self.config))
    }

    fn try_load_mmdb(&mut self, path: Option<PathBuf>) -> Result<bool, String> {
        let use_mmdb = matches!(
            self.config.source.as_str(),
            "auto" | "mmdb"
        );
        if !use_mmdb {
            return Ok(false);
        }
        let Some(path) = path else {
            return Ok(false);
        };
        if !path.is_file() {
            tracing::warn!("GeoIP MMDB not found: {}", path.display());
            return Ok(false);
        }
        let reader = Reader::open_readfile(&path).map_err(|e: MaxMindDbError| e.to_string())?;
        self.mmdb = Some(Arc::new(reader));
        self.mmdb_path = Some(path);
        tracing::info!(
            "GeoIP MMDB loaded: {}",
            self.mmdb_path.as_ref().unwrap().display()
        );
        Ok(true)
    }

    pub fn lookup(&self, ip: &str) -> GeoLookupResult {
        let parsed: IpAddr = match ip.parse() {
            Ok(a) => a,
            Err(_) => {
                return GeoLookupResult {
                    ip: ip.into(),
                    country: None,
                    allowed: true,
                    reason: Some("invalid_ip".into()),
                    source: None,
                };
            }
        };

        let (country, lookup_source) = self.resolve_country(parsed);

        if !self.config.enabled {
            return GeoLookupResult {
                ip: ip.into(),
                country,
                allowed: true,
                reason: None,
                source: lookup_source,
            };
        }

        let cc = country.as_deref().unwrap_or("XX").to_string();
        if !self.config.allow_countries.is_empty() {
            let ok = self
                .config
                .allow_countries
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&cc));
            return GeoLookupResult {
                ip: ip.into(),
                country,
                allowed: ok,
                reason: if ok {
                    None
                } else {
                    Some("country_not_allowed".into())
                },
                source: lookup_source,
            };
        }

        let blocked = self
            .config
            .block_countries
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&cc));
        GeoLookupResult {
            ip: ip.into(),
            country,
            allowed: !blocked,
            reason: if blocked {
                Some(format!("country_blocked:{cc}"))
            } else {
                None
            },
            source: lookup_source,
        }
    }

    fn resolve_country(&self, ip: IpAddr) -> (Option<String>, Option<String>) {
        let use_mmdb = matches!(self.config.source.as_str(), "auto" | "mmdb");
        if use_mmdb {
            if let Some(ref reader) = self.mmdb {
                if let Ok(result) = reader.lookup(ip) {
                    if let Ok(Some(country)) = result.decode::<geoip2::Country>() {
                        if let Some(iso) = country.country.iso_code {
                            return (Some(iso.to_uppercase()), Some("mmdb".into()));
                        }
                    }
                }
            }
        }

        if self.config.source == "mmdb" {
            return (None, Some("mmdb".into()));
        }

        let country = self
            .rules
            .iter()
            .find(|(n, _)| n.contains(&ip))
            .map(|(_, c)| c.clone());
        (country, Some("cidr".into()))
    }
}

fn resolve_mmdb_path(config: &GeoConfig) -> Option<PathBuf> {
    if let Ok(env) = std::env::var("GEOIP_DB_PATH") {
        if !env.is_empty() {
            return Some(PathBuf::from(env));
        }
    }
    config
        .mmdb_path
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(default_mmdb_candidates)
}

fn default_mmdb_candidates() -> Option<PathBuf> {
    for candidate in [
        "rules/geoip/GeoLite2-Country.mmdb",
        "data/geoip/GeoLite2-Country.mmdb",
        "/usr/share/GeoIP/GeoLite2-Country.mmdb",
        "/var/lib/GeoIP/GeoLite2-Country.mmdb",
    ] {
        let p = Path::new(candidate);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    None
}

fn builtin_cidrs() -> Vec<GeoCidrRule> {
    vec![
        GeoCidrRule {
            cidr: "10.0.0.0/8".into(),
            country: "PRIVATE".into(),
        },
        GeoCidrRule {
            cidr: "172.16.0.0/12".into(),
            country: "PRIVATE".into(),
        },
        GeoCidrRule {
            cidr: "192.168.0.0/16".into(),
            country: "PRIVATE".into(),
        },
        GeoCidrRule {
            cidr: "127.0.0.0/8".into(),
            country: "PRIVATE".into(),
        },
    ]
}

impl Default for GeoEngine {
    fn default() -> Self {
        Self::with_builtin(GeoConfig::default())
    }
}

#[cfg(test)]
mod test;
