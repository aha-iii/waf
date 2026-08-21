//! CRS runtime config and capability matrix (Console / API).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static WASM_LOADED: AtomicBool = AtomicBool::new(false);

/// Called by engine after successful WASM init.
pub fn set_wasm_loaded(loaded: bool) {
    WASM_LOADED.store(loaded, Ordering::SeqCst);
}

pub fn wasm_loaded_flag() -> bool {
    WASM_LOADED.load(Ordering::SeqCst)
}

pub fn resolve_wasm_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NERVE_WAF_WASM_CRS_PATH") {
        let p = PathBuf::from(p.trim());
        if p.is_file() {
            return Some(p);
        }
        tracing::warn!(path = %p.display(), "NERVE_WAF_WASM_CRS_PATH not found");
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("../../wasm/coraza-proxy-wasm.wasm"),
        PathBuf::from("wasm/coraza-proxy-wasm.wasm"),
        PathBuf::from("Nerve-waf/wasm/coraza-proxy-wasm.wasm"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// User-configurable CRS engine preference (written to `data/crs_runtime.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrsEnginePreference {
    /// Prefer WASM when available; else fall back to libinjection.
    Auto,
    /// Single-process WASM Coraza+CRS.
    Wasm,
    /// Skip WASM; crs stage uses libinjection (or Go Coraza if configured).
    Off,
    /// Go HTTP sidecar process (experimental).
    Coraza,
}

impl Default for CrsEnginePreference {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrsRuntimeConfig {
    pub version: u32,
    #[serde(default)]
    pub engine: CrsEnginePreference,
}

impl Default for CrsRuntimeConfig {
    fn default() -> Self {
        Self {
            version: 1,
            engine: CrsEnginePreference::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrsCapability {
    pub key: String,
    pub label: String,
    /// Whether effective under the currently selected engine.
    pub applies: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveCrsEngine {
    Wasm,
    CorazaHttp,
    Libinjection,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrsRuntimeStatus {
    pub preference: CrsEnginePreference,
    pub preference_source: String,
    pub env_override: Option<String>,
    pub effective_engine: EffectiveCrsEngine,
    pub wasm_available: bool,
    pub wasm_loaded: bool,
    pub wasm_path: Option<String>,
    pub capabilities: Vec<CrsCapability>,
}

pub fn crs_runtime_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("NERVE_WAF_CRS_RUNTIME_CONFIG") {
        return PathBuf::from(p.trim());
    }
    let data = std::env::var("NERVE_WAF_DATA")
        .or_else(|_| std::env::var("NERVE_WAF_DATA_DIR"))
        .unwrap_or_else(|_| "data".into());
    PathBuf::from(data).join("crs_runtime.json")
}

pub fn load_runtime_config() -> CrsRuntimeConfig {
    let path = crs_runtime_config_path();
    if !path.is_file() {
        return CrsRuntimeConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "read crs_runtime.json failed");
            CrsRuntimeConfig::default()
        }
    }
}

pub fn save_runtime_config(cfg: &CrsRuntimeConfig) -> Result<(), String> {
    let path = crs_runtime_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())?;
    Ok(())
}

fn env_engine_override() -> Option<String> {
    std::env::var("NERVE_WAF_CRS_ENGINE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn preference_from_env(env: &str) -> Option<CrsEnginePreference> {
    match env.trim().to_ascii_lowercase().as_str() {
        "wasm" | "proxy-wasm" | "crs-wasm" | "coraza-wasm" | "default" => {
            Some(CrsEnginePreference::Wasm)
        }
        "coraza" | "crs-coraza" | "full" => Some(CrsEnginePreference::Coraza),
        "off" | "none" | "libinjection" | "rust" => Some(CrsEnginePreference::Off),
        "auto" => Some(CrsEnginePreference::Auto),
        _ => None,
    }
}

pub fn resolved_preference() -> (CrsEnginePreference, &'static str) {
    if let Some(env) = env_engine_override() {
        if let Some(p) = preference_from_env(&env) {
            return (p, "env");
        }
    }
    let file = load_runtime_config();
    (file.engine, "config")
}

pub fn effective_engine() -> EffectiveCrsEngine {
    let (pref, _) = resolved_preference();
    let wasm_ok = resolve_wasm_path().is_some();
    match pref {
        CrsEnginePreference::Wasm => {
            if wasm_ok {
                EffectiveCrsEngine::Wasm
            } else {
                EffectiveCrsEngine::Libinjection
            }
        }
        CrsEnginePreference::Coraza => EffectiveCrsEngine::CorazaHttp,
        CrsEnginePreference::Off => EffectiveCrsEngine::Libinjection,
        CrsEnginePreference::Auto => {
            if wasm_ok {
                EffectiveCrsEngine::Wasm
            } else if coraza_http_enabled() {
                EffectiveCrsEngine::CorazaHttp
            } else {
                EffectiveCrsEngine::Libinjection
            }
        }
    }
}

pub fn runtime_status() -> CrsRuntimeStatus {
    let (preference, preference_source) = resolved_preference();
    let env_override = env_engine_override();
    let effective = effective_engine();
    let wasm_available = resolve_wasm_path().is_some();
    let wasm_path = resolve_wasm_path().map(|p| p.display().to_string());
    let wasm_loaded = wasm_loaded_flag();

    let is_wasm = matches!(effective, EffectiveCrsEngine::Wasm);
    let is_rust_l7 = matches!(
        effective,
        EffectiveCrsEngine::Libinjection | EffectiveCrsEngine::Disabled
    );

    let capabilities = vec![
        CrsCapability {
            key: "pipeline_crs_stage".into(),
            label: "流水线核心/专家阶段开关".into(),
            applies: true,
            note: "应用配置 → 检测流水线 → 专家防护 / 核心防护".into(),
        },
        CrsCapability {
            key: "engine_selector".into(),
            label: "专家防护引擎选择".into(),
            applies: env_override.is_none(),
            note: if env_override.is_some() {
                "当前被运维环境变量覆盖".into()
            } else {
                "本页可切换；保存后需重载边缘进程或热同步".into()
            },
        },
        CrsCapability {
            key: "module_category_toggle".into(),
            label: "sqli/xss 等模块开关".into(),
            applies: !is_wasm,
            note: if is_wasm {
                "专家防护为整包规则集；模块开关仅作用于核心防护段".into()
            } else {
                "对核心防护路径生效".into()
            },
        },
        CrsCapability {
            key: "disabled_rules".into(),
            label: "单规则禁用 (disabled_rules)".into(),
            applies: !is_wasm,
            note: if is_wasm {
                "专家防护不开放按规则 ID 关闭".into()
            } else {
                "对核心防护路径生效".into()
            },
        },
        CrsCapability {
            key: "app_crs_binding".into(),
            label: "应用级规则版本绑定".into(),
            applies: !is_wasm,
            note: if is_wasm {
                "专家防护使用平台内置规则集，不读应用绑定快照".into()
            } else {
                "绑定后影响本应用核心防护规则合并".into()
            },
        },
        CrsCapability {
            key: "crs_rules_admin".into(),
            label: "核心规则库热加载/回滚".into(),
            applies: !is_wasm || is_rust_l7,
            note: "维护核心防护规则库；专家防护使用独立受保护资产".into(),
        },
        CrsCapability {
            key: "edge_modules_sync".into(),
            label: "edge apply_modules 热同步".into(),
            applies: true,
            note: if is_wasm {
                "同步 modules 配置；专家防护引擎本身不随模块重配".into()
            } else {
                "同步 modules 并更新核心防护".into()
            },
        },
    ];

    CrsRuntimeStatus {
        preference,
        preference_source: preference_source.into(),
        env_override,
        effective_engine: effective,
        wasm_available,
        wasm_loaded,
        wasm_path,
        capabilities,
    }
}

fn coraza_http_enabled() -> bool {
    if std::env::var("NERVE_WAF_CORAZA_URL")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    matches!(
        std::env::var("NERVE_WAF_CRS_ENGINE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "coraza" | "crs-coraza" | "full"
    )
}
