//! 进程内 **Coraza + OWASP CRS**（`coraza-proxy-wasm.wasm`，Proxy-Wasm ABI）。
//!
//! Gateway `mode=edge` 单进程加载 WASM，无需 Go `coraza-engine` 伴生进程。
//!
//! - `NERVE_WAF_CRS_ENGINE=wasm`（默认，当 wasm 文件存在）
//! - `NERVE_WAF_WASM_CRS_PATH`：`.wasm` 路径（默认 `Nerve-waf/wasm/coraza-proxy-wasm.wasm`）
//! - `NERVE_WAF_WASM_CRS_CONFIG`：Coraza 插件 JSON 配置文件（可选）

mod runtime;
mod engine;

pub use engine::{inspect_blocking, wasm_crs_enabled, wasm_crs_loaded, WasmCrsError};
pub use runtime::{
    load_runtime_config, resolve_wasm_path, runtime_status, save_runtime_config, set_wasm_loaded,
    CrsCapability, CrsEnginePreference, CrsRuntimeConfig, CrsRuntimeStatus, EffectiveCrsEngine,
};
