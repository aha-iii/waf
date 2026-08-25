use nerve_waf_l7::{InspectRequest, InspectResult};
use proxy_wasm_test_framework::host_settings::LocalBlockSignal;
use proxy_wasm_test_framework::tester::{self, MockSettings, Tester};
use proxy_wasm_test_framework::types::{Action, BufferType, MapType, ReturnType};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::runtime::{resolve_wasm_path, set_wasm_loaded};

const ROOT_CONTEXT: i32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum WasmCrsError {
    #[error("wasm crs disabled")]
    Disabled,
    #[error("init: {0}")]
    Init(String),
    #[error("inspect: {0}")]
    Inspect(String),
}

struct CorazaWasmEngine {
    tester: Tester,
    next_http_ctx: AtomicI32,
}

impl CorazaWasmEngine {
    fn new(wasm_path: &Path, config_json: &str) -> Result<Self, String> {
        let mut tester = tester::mock(MockSettings {
            wasm_path: wasm_path.to_string_lossy().into_owned(),
            quiet: true,
            allow_unexpected: true,
        })
        .map_err(|e| e.to_string())?;

        tester.set_quiet(true);
        tester.reset_host_settings();
        tester.toggle_strict_mode(false);

        tester.call_start();
        tester
            .execute_and_expect(ReturnType::None)
            .map_err(|e| e.to_string())?;

        tester.call_proxy_on_context_create(ROOT_CONTEXT, 0);
        tester
            .execute_and_expect(ReturnType::None)
            .map_err(|e| e.to_string())?;

        tester
            .set_default_buffer_bytes(BufferType::PluginConfiguration)
            .returning(config_json);
        tester.call_proxy_on_vm_start(0, 0);
        tester
            .execute_and_expect(ReturnType::Bool(true))
            .map_err(|e| e.to_string())?;

        tester
            .set_default_buffer_bytes(BufferType::PluginConfiguration)
            .returning(config_json);
        tester
            .call_proxy_on_configure(ROOT_CONTEXT, config_json.len() as i32);
        tester
            .execute_and_expect(ReturnType::Bool(true))
            .map_err(|e| e.to_string())?;

        Ok(Self {
            tester,
            next_http_ctx: AtomicI32::new(2),
        })
    }

    fn inspect(&mut self, req: &InspectRequest) -> Result<Option<InspectResult>, String> {
        let http_ctx = self.next_http_ctx.fetch_add(1, Ordering::Relaxed);
        self.tester.clear_block_signal();

        self.tester.call_proxy_on_context_create(http_ctx, ROOT_CONTEXT);
        self.tester
            .execute_and_expect(ReturnType::None)
            .map_err(|e| e.to_string())?;

        let headers = build_request_headers(req);
        let num_headers = headers.len() as i32;
        let body = req.body.as_str();
        let has_body = !body.is_empty();

        let header_pairs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        self.tester
            .set_default_header_map_pairs(MapType::HttpRequestHeaders)
            .returning(header_pairs);

        let hdr_action = self
            .tester
            .run_request_headers_raw(http_ctx, num_headers, !has_body)
            .map_err(|e| e.to_string())?;

        if let Some(hit) = block_from_signal(&mut self.tester, req, hdr_action) {
            self.finish_http_context(http_ctx);
            return Ok(Some(hit));
        }

        if has_body {
            self.tester
                .set_default_buffer_bytes(BufferType::HttpRequestBody)
                .returning(body);
            let body_action = self
                .tester
                .run_request_body_raw(http_ctx, body.len() as i32, true)
                .map_err(|e| e.to_string())?;

            if let Some(hit) = block_from_signal(&mut self.tester, req, body_action) {
                self.finish_http_context(http_ctx);
                return Ok(Some(hit));
            }
        }

        self.finish_http_context(http_ctx);
        Ok(None)
    }

    fn finish_http_context(&mut self, http_ctx: i32) {
        self.tester.call_proxy_on_delete(http_ctx);
        let _ = self.tester.execute_and_expect(ReturnType::None);
        self.tester.clear_block_signal();
    }
}

fn block_from_signal(
    tester: &mut Tester,
    req: &InspectRequest,
    action: i32,
) -> Option<InspectResult> {
    let signal = tester
        .take_block_signal()
        .or_else(|| {
            if action == Action::Pause as i32 {
                Some(LocalBlockSignal {
                    status_code: 403,
                    body: String::new(),
                })
            } else {
                None
            }
        })?;

    Some(signal_to_hit(req, &signal))
}

fn signal_to_hit(req: &InspectRequest, signal: &LocalBlockSignal) -> InspectResult {
    let blocked = signal.status_code >= 400 || signal.status_code == 0;
    InspectResult {
        blocked,
        action: if blocked { "block".into() } else { "pass".into() },
        category: "expert".into(),
        rule_id: extract_rule_id(&signal.body).unwrap_or_else(|| "expert".into()),
        rule_name: if signal.body.is_empty() {
            "专家防护".into()
        } else {
            truncate_line(&signal.body, 120)
        },
        severity: "critical".into(),
        matched_field: "request".into(),
        payload_sample: truncate_line(&req.uri, 160),
        plugin_name: Some("expert".into()),
        plugin_type: Some("expert".into()),
        param_path: None,
        decode_chain: None,
        semantic_score: None,
        engines: Some(vec!["expert".into()]),
    }
}

fn extract_rule_id(body: &str) -> Option<String> {
    for token in body.split(|c: char| !c.is_ascii_digit()) {
        if token.len() >= 5 && token.starts_with('9') {
            return Some(token.to_string());
        }
    }
    None
}

fn truncate_line(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

fn build_request_headers(req: &InspectRequest) -> Vec<(String, String)> {
    let mut hdrs = vec![
        (":method".into(), req.method.clone()),
        (":authority".into(), req.host.clone()),
        (":path".into(), req.uri.clone()),
    ];
    if !req.user_agent.is_empty() {
        hdrs.push(("user-agent".into(), req.user_agent.clone()));
    }
    if !req.src_ip.is_empty() {
        hdrs.push(("x-forwarded-for".into(), req.src_ip.clone()));
    }
    for (k, v) in parse_headers_map(&req.headers) {
        let lk = k.to_ascii_lowercase();
        if lk == ":method" || lk == ":authority" || lk == ":path" || lk == "host" {
            continue;
        }
        hdrs.push((lk, v));
    }
    if !req.body.is_empty() && !hdrs.iter().any(|(k, _)| k == "content-length") {
        hdrs.push(("content-length".into(), req.body.len().to_string()));
    }
    hdrs
}

fn parse_headers_map(headers: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let t = headers.trim();
    if t.is_empty() {
        return map;
    }
    if t.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
            if let Some(obj) = v.as_object() {
                for (k, val) in obj {
                    let s = val
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| val.to_string());
                    map.insert(k.clone(), s);
                }
            }
        }
        return map;
    }
    for line in t.lines() {
        if let Some((k, v)) = line.split_once(':') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn default_coraza_config() -> String {
    r#"{"directives_map":{"default":["Include @recommended-conf","SecRuleEngine On","Include @crs-setup-conf","Include @owasp_crs/*.conf"]},"default_directives":"default"}"#.into()
}

fn load_config_json() -> String {
    if let Ok(path) = std::env::var("NERVE_WAF_WASM_CRS_CONFIG") {
        let path = path.trim();
        if !path.is_empty() {
            if let Ok(raw) = std::fs::read_to_string(path) {
                return raw;
            }
            tracing::warn!(path, "NERVE_WAF_WASM_CRS_CONFIG unreadable, using built-in CRS");
        }
    }
    default_coraza_config()
}

/// Whether WASM CRS path is selected.
pub fn wasm_crs_enabled() -> bool {
    matches!(
        crate::runtime::effective_engine(),
        crate::runtime::EffectiveCrsEngine::Wasm
    )
}

/// True when WASM engine has been initialized successfully.
pub fn wasm_crs_loaded() -> bool {
    crate::runtime::wasm_loaded_flag()
}

fn engine() -> Result<&'static Mutex<CorazaWasmEngine>, WasmCrsError> {
    static ENGINE: OnceLock<Result<Mutex<CorazaWasmEngine>, String>> = OnceLock::new();
    let slot = ENGINE.get_or_init(|| {
        let path = resolve_wasm_path()
            .ok_or_else(|| String::from("coraza-proxy-wasm.wasm not found"))?;
        let config = load_config_json();
        let result = CorazaWasmEngine::new(&path, &config).map(Mutex::new);
        set_wasm_loaded(result.is_ok());
        result
    });
    slot.as_ref()
        .map_err(|e| WasmCrsError::Init(e.clone()))
}

/// Blocking inspect for embed / sync pipeline paths.
pub fn inspect_blocking(req: &InspectRequest) -> Result<Option<InspectResult>, WasmCrsError> {
    if !wasm_crs_enabled() {
        return Err(WasmCrsError::Disabled);
    }
    let eng = engine()?;
    let mut guard = eng.lock().map_err(|_| WasmCrsError::Inspect("lock poisoned".into()))?;
    guard
        .inspect(req)
        .map_err(WasmCrsError::Inspect)
}
