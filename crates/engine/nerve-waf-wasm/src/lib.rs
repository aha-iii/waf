//! In-process Coraza/Proxy-WASM CRS engine bridge.
//!
//! Loads `coraza-proxy-wasm.wasm` in-process (no companion Go Coraza process).
//!
//! - `NERVE_WAF_CRS_ENGINE=wasm` (default when the wasm file is present)
//! - `NERVE_WAF_WASM_CRS_PATH` - path to the `.wasm` (default under `Nerve-waf/wasm/`)
//! - `NERVE_WAF_WASM_CRS_CONFIG` - optional Coraza plugin JSON config

mod engine;
mod runtime;

pub use engine::{inspect_blocking, wasm_crs_enabled, wasm_crs_loaded, WasmCrsError};
pub use runtime::{
    load_runtime_config, resolve_wasm_path, runtime_status, save_runtime_config, set_wasm_loaded,
    CrsCapability, CrsEnginePreference, CrsRuntimeConfig, CrsRuntimeStatus, EffectiveCrsEngine,
};

#[cfg(test)]
mod test;
