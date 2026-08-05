use mlua::{Lua, Result as LuaResult, Table, Value};
use nerve_waf_l7::{InspectRequest, InspectResult};
use nerve_waf_sdk::PluginManifest;
use std::path::{Path, PathBuf};

pub struct LuaInspectPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
    pub script_path: PathBuf,
}

impl LuaInspectPlugin {
    pub fn from_manifest(dir: &Path, manifest: PluginManifest) -> Result<Self, String> {
        let script_rel = manifest
            .script
            .clone()
            .unwrap_or_else(|| "plugin.lua".into());
        let script_path = dir.join(&script_rel);
        if !script_path.is_file() {
            return Err(format!(
                "lua plugin {} missing script {}",
                manifest.name,
                script_path.display()
            ));
        }
        Ok(Self {
            manifest,
            dir: dir.to_path_buf(),
            script_path,
        })
    }

    pub fn inspect(&self, req: &InspectRequest, modules_mode: &str) -> Result<Option<InspectResult>, String> {
        let lua = Lua::new();
        let script = std::fs::read_to_string(&self.script_path)
            .map_err(|e| format!("read lua {}: {e}", self.script_path.display()))?;

        lua.load(&script)
            .set_name(self.script_path.to_string_lossy().into_owned())
            .exec()
            .map_err(|e| format!("lua load {}: {e}", self.manifest.name))?;

        let globals = lua.globals();
        let inspect_fn: mlua::Function = globals
            .get("inspect")
            .map_err(|_| format!("lua plugin {} must define inspect(req)", self.manifest.name))?;

        let req_table = lua_req_table(&lua, req).map_err(|e| format!("lua req table {}: {e}", self.manifest.name))?;
        let ret: Value = inspect_fn
            .call(req_table)
            .map_err(|e| format!("lua inspect {}: {e}", self.manifest.name))?;

        parse_lua_hit(ret, &self.manifest, modules_mode)
    }
}

fn lua_req_table(lua: &Lua, req: &InspectRequest) -> LuaResult<Table> {
    let t = lua.create_table()?;
    t.set("method", req.method.clone())?;
    t.set("host", req.host.clone())?;
    t.set("uri", req.uri.clone())?;
    t.set("headers", req.headers.clone())?;
    t.set("body", req.body.clone())?;
    t.set("src_ip", req.src_ip.clone())?;
    t.set("user_agent", req.user_agent.clone())?;
    Ok(t)
}

fn parse_lua_hit(
    value: Value,
    manifest: &PluginManifest,
    modules_mode: &str,
) -> Result<Option<InspectResult>, String> {
    match value {
        Value::Nil => Ok(None),
        Value::Table(t) => {
            let blocked: bool = t.get("blocked").unwrap_or(false);
            let mut action: String = t.get("action").unwrap_or_else(|_| {
                if blocked {
                    "block".into()
                } else {
                    "log".into()
                }
            });
            if modules_mode == "detect" {
                action = "log".into();
            }
            let category: String = t
                .get("category")
                .unwrap_or_else(|_| manifest.category.clone().unwrap_or_else(|| "custom".into()));
            let rule_id: String = t.get("rule_id").unwrap_or_else(|_| format!("lua:{}", manifest.name));
            let rule_name: String = t.get("rule_name").unwrap_or_else(|_| manifest.name.clone());
            let severity: String = t.get("severity").unwrap_or_else(|_| "medium".into());
            let matched_field: String = t.get("matched_field").unwrap_or_else(|_| "lua".into());
            let payload_sample: String = t.get("payload_sample").unwrap_or_default();
            Ok(Some(InspectResult {
                blocked: blocked && action == "block",
                action,
                category,
                rule_id,
                rule_name,
                severity,
                matched_field,
                payload_sample,
                plugin_name: Some(manifest.name.clone()),
                plugin_type: Some("lua".into()),
                param_path: None,
                decode_chain: None,
                semantic_score: None,
                engines: None,
            }))
        }
        other => Err(format!(
            "lua plugin {} inspect() must return nil or table, got {:?}",
            manifest.name, other
        )),
    }
}
