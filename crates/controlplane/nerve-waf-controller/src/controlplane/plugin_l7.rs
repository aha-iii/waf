//! L7 JSON / Lua plugin CRUD + versioning (plugins/l7/<name>/ + versions/).

use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use nerve_waf_sdk::PluginManifest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

const MAX_VERSIONS: usize = 20;

pub fn plugin_l7_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/plugins/l7",
            get(list_l7_plugins).post(create_l7_plugin),
        )
        .route(
            "/api/plugins/l7/{name}",
            get(get_l7_plugin)
                .put(put_l7_plugin)
                .delete(delete_l7_plugin),
        )
        .route("/api/plugins/l7/{name}/publish", post(publish_l7_plugin))
        .route("/api/plugins/l7/{name}/rollback", post(rollback_l7_plugin))
        .route("/api/plugins/l7/{name}/versions", get(list_l7_versions))
        .route(
            "/api/plugins/l7/{name}/versions/{version}",
            get(get_l7_version),
        )
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(rename = "type")]
    plugin_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct L7PluginSummary {
    name: String,
    version: String,
    description: String,
    #[serde(rename = "type")]
    plugin_type: String,
    category: Option<String>,
    priority: i32,
    enabled: bool,
    path: String,
    rule_file_count: usize,
    has_script: bool,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    name: String,
    #[serde(rename = "type")]
    plugin_type: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default = "default_priority")]
    priority: i32,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    rules_json: Option<String>,
    #[serde(default)]
    script: Option<String>,
}

fn default_priority() -> i32 {
    100
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct PutBody {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    rules_json: Option<String>,
    #[serde(default)]
    script: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublishBody {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    rules_json: Option<String>,
    #[serde(default)]
    script: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RollbackBody {
    version: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionMeta {
    published_at: String,
    #[serde(default)]
    note: String,
}

fn validate_name(name: &str) -> Result<(), String> {
    let ok = name.len() <= 64
        && name
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphanumeric() || c == '-' || c == '_' || (i > 0 && c == '.'));
    if !ok || name.is_empty() || name.contains("..") {
        return Err("插件名仅允许字母数字、短横线、下划线".into());
    }
    Ok(())
}

fn validate_version(ver: &str) -> Result<String, String> {
    let v = ver.trim().trim_start_matches('v').trim().to_string();
    if v.is_empty()
        || v.contains("..")
        || v.contains('/')
        || v.contains('\\')
        || !v
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err("非法版本号".into());
    }
    Ok(v)
}

fn version_dir_name(ver: &str) -> String {
    format!("v{}", ver.trim().trim_start_matches('v'))
}

fn bump_patch(ver: &str) -> String {
    let v = ver.trim().trim_start_matches('v');
    let parts: Vec<&str> = v.split('.').collect();
    let major: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    format!("{major}.{minor}.{}", patch + 1)
}

fn now_rfc3339() -> String {
    // Without chrono: rough UTC from SystemTime → ISO-ish.
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn plugins_root(state: &AppState) -> PathBuf {
    state
        .plugin_host
        .read()
        .unwrap()
        .plugins_dir()
        .to_path_buf()
}

fn plugin_dir(root: &FsPath, name: &str) -> PathBuf {
    root.join("l7").join(name)
}

fn versions_root(dir: &FsPath) -> PathBuf {
    dir.join("versions")
}

fn read_manifest(dir: &FsPath) -> Result<PluginManifest, String> {
    let path = dir.join("plugin.json");
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读 plugin.json: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析 plugin.json: {e}"))
}

fn write_manifest(dir: &FsPath, m: &PluginManifest) -> Result<(), String> {
    let path = dir.join("plugin.json");
    let raw = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw + "\n").map_err(|e| format!("写 plugin.json: {e}"))
}

fn count_rule_files(dir: &FsPath) -> usize {
    let rules = dir.join("rules");
    if !rules.is_dir() {
        return 0;
    }
    std::fs::read_dir(&rules)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                })
                .count()
        })
        .unwrap_or(0)
}

fn load_rules_json_concat(dir: &FsPath) -> Result<String, String> {
    let rules_dir = dir.join("rules");
    if !rules_dir.is_dir() {
        return Ok("[]".into());
    }
    let mut all: Vec<Value> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&rules_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let raw = std::fs::read_to_string(e.path()).map_err(|err| err.to_string())?;
        let v: Value =
            serde_json::from_str(&raw).map_err(|err| format!("{}: {err}", e.path().display()))?;
        match v {
            Value::Array(arr) => all.extend(arr),
            other => all.push(other),
        }
    }
    serde_json::to_string_pretty(&all).map_err(|e| e.to_string())
}

fn write_rules_json(dir: &FsPath, rules_raw: &str) -> Result<(), String> {
    let _: Value = serde_json::from_str(rules_raw).map_err(|e| format!("rulesJson 非法: {e}"))?;
    let rules_dir = dir.join("rules");
    std::fs::create_dir_all(&rules_dir).map_err(|e| e.to_string())?;
    if let Ok(rd) = std::fs::read_dir(&rules_dir) {
        for e in rd.filter_map(|x| x.ok()) {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    std::fs::write(rules_dir.join("rules.json"), rules_raw.to_string() + "\n")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn file_mtime_iso(path: &FsPath) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let secs = modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(secs.to_string())
}

fn read_version_meta(vdir: &FsPath) -> VersionMeta {
    let path = vdir.join("meta.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(m) = serde_json::from_str::<VersionMeta>(&raw) {
            return m;
        }
    }
    VersionMeta {
        published_at: file_mtime_iso(vdir).unwrap_or_else(now_rfc3339),
        note: String::new(),
    }
}

fn write_version_meta(vdir: &FsPath, note: &str) -> Result<(), String> {
    let meta = VersionMeta {
        published_at: now_rfc3339(),
        note: note.to_string(),
    };
    let raw = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    std::fs::write(vdir.join("meta.json"), raw + "\n").map_err(|e| e.to_string())
}

/// Copy current dir (excluding versions/) to versions/vX.Y.Z/.
fn archive_current(dir: &FsPath, version: &str, note: &str) -> Result<(), String> {
    let ver = validate_version(version)?;
    let vroot = versions_root(dir);
    let vdir = vroot.join(version_dir_name(&ver));
    if vdir.is_dir() {
        // Do not overwrite existing history.
        return Ok(());
    }
    std::fs::create_dir_all(&vdir).map_err(|e| e.to_string())?;

    // plugin.json
    let src_manifest = dir.join("plugin.json");
    if src_manifest.is_file() {
        std::fs::copy(&src_manifest, vdir.join("plugin.json")).map_err(|e| e.to_string())?;
    }

    let m = read_manifest(dir).ok();
    let ptype = m
        .as_ref()
        .map(|x| x.plugin_type.to_lowercase())
        .unwrap_or_default();

    if ptype == "lua" {
        let script_file = m
            .as_ref()
            .and_then(|x| x.script.clone())
            .unwrap_or_else(|| "plugin.lua".into());
        let src = dir.join(&script_file);
        if src.is_file() {
            std::fs::copy(&src, vdir.join(&script_file)).map_err(|e| e.to_string())?;
        }
    } else {
        let src_rules = dir.join("rules");
        if src_rules.is_dir() {
            let dst_rules = vdir.join("rules");
            std::fs::create_dir_all(&dst_rules).map_err(|e| e.to_string())?;
            if let Ok(rd) = std::fs::read_dir(&src_rules) {
                for e in rd.filter_map(|x| x.ok()) {
                    let p = e.path();
                    if p.is_file() {
                        let name = e.file_name();
                        std::fs::copy(&p, dst_rules.join(name)).map_err(|err| err.to_string())?;
                    }
                }
            }
        }
    }

    write_version_meta(&vdir, note)?;
    prune_versions(dir)?;
    Ok(())
}

fn prune_versions(dir: &FsPath) -> Result<(), String> {
    let vroot = versions_root(dir);
    if !vroot.is_dir() {
        return Ok(());
    }
    let mut dirs: Vec<(String, PathBuf, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&vroot) {
        for e in rd.filter_map(|x| x.ok()) {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with('v') {
                continue;
            }
            let meta = read_version_meta(&p);
            dirs.push((meta.published_at, p, name));
        }
    }
    dirs.sort_by(|a, b| b.0.cmp(&a.0).then(b.2.cmp(&a.2)));
    for (_, path, _) in dirs.into_iter().skip(MAX_VERSIONS) {
        let _ = std::fs::remove_dir_all(path);
    }
    Ok(())
}

fn current_updated_at(dir: &FsPath, version: &str) -> Option<String> {
    let vdir = versions_root(dir).join(version_dir_name(version));
    if vdir.is_dir() {
        return Some(read_version_meta(&vdir).published_at);
    }
    file_mtime_iso(&dir.join("plugin.json"))
}

fn detail_json(dir: &FsPath) -> Result<Value, String> {
    let m = read_manifest(dir)?;
    let ptype = m.plugin_type.to_lowercase();
    let mut body = json!({
        "manifest": m,
        "path": dir.display().to_string(),
        "type": ptype,
        "updatedAt": current_updated_at(dir, &m.version),
    });
    if ptype == "json" {
        body["rulesJson"] = Value::String(load_rules_json_concat(dir)?);
    } else if ptype == "lua" {
        let script_file = m.script.clone().unwrap_or_else(|| "plugin.lua".into());
        let script = std::fs::read_to_string(dir.join(&script_file)).unwrap_or_default();
        body["script"] = Value::String(script);
        body["scriptFile"] = Value::String(script_file);
    }
    Ok(body)
}

fn map_err_status(e: String, code: StatusCode) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": e })))
}

fn scan_summaries(root: &FsPath, type_filter: Option<&str>) -> Vec<L7PluginSummary> {
    let l7 = root.join("l7");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&l7) else {
        return out;
    };
    for e in rd.filter_map(|x| x.ok()) {
        if !e.path().is_dir() {
            continue;
        }
        let dir = e.path();
        let Ok(m) = read_manifest(&dir) else {
            continue;
        };
        let ptype = m.plugin_type.to_lowercase();
        if ptype != "json" && ptype != "lua" {
            continue;
        }
        if let Some(f) = type_filter {
            if ptype != f.to_lowercase() {
                continue;
            }
        }
        let script_name = m.script.clone().unwrap_or_else(|| "plugin.lua".into());
        out.push(L7PluginSummary {
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            plugin_type: ptype.clone(),
            category: m.category.clone(),
            priority: m.priority,
            enabled: m.enabled,
            path: dir.display().to_string(),
            rule_file_count: if ptype == "json" {
                count_rule_files(&dir)
            } else {
                0
            },
            has_script: ptype == "lua" && dir.join(&script_name).is_file(),
            updated_at: current_updated_at(&dir, &m.version),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

async fn list_l7_plugins(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Json<Value> {
    let root = plugins_root(&state);
    let filter = q.plugin_type.as_deref();
    let plugins = scan_summaries(&root, filter);
    Json(json!({ "plugins": plugins }))
}

async fn get_l7_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    let body = detail_json(&dir).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    Ok(Json(body))
}

async fn create_l7_plugin(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let name = body.name.trim().to_string();
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let ptype = body.plugin_type.to_lowercase();
    if ptype != "json" && ptype != "lua" {
        return Err(map_err_status(
            "type 须为 json 或 lua".into(),
            StatusCode::BAD_REQUEST,
        ));
    }
    let root = plugins_root(&state);
    let dir = plugin_dir(&root, &name);
    if dir.exists() {
        return Err(map_err_status("插件已存在".into(), StatusCode::CONFLICT));
    }
    std::fs::create_dir_all(&dir).map_err(|e| {
        map_err_status(
            format!("创建目录失败: {e}"),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
    })?;

    let mut manifest = PluginManifest {
        name: name.clone(),
        layer: "l7".into(),
        plugin_type: ptype.clone(),
        version: "1.0.0".into(),
        description: body.description,
        priority: body.priority,
        category: body.category,
        rules: Vec::new(),
        script: None,
        enabled: body.enabled,
    };

    if ptype == "json" {
        let rules_raw = body.rules_json.unwrap_or_else(|| "[]".into());
        write_rules_json(&dir, &rules_raw)
            .map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
        manifest.rules = vec!["rules/*.json".into()];
    } else {
        let script = body
            .script
            .unwrap_or_else(|| "function inspect(req)\n  return nil\nend\n".into());
        std::fs::write(dir.join("plugin.lua"), script)
            .map_err(|e| map_err_status(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?;
        manifest.script = Some("plugin.lua".into());
    }

    write_manifest(&dir, &manifest)
        .map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    archive_current(&dir, "1.0.0", "initial")
        .map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    let report = state.reload_plugins_and_l7_async().await;
    Ok(Json(json!({
        "ok": true,
        "name": name,
        "version": "1.0.0",
        "report": report
    })))
}

async fn put_l7_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<PutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    if body.rules_json.is_some() || body.script.is_some() {
        return Err(map_err_status(
            "内容变更请使用发布接口".into(),
            StatusCode::BAD_REQUEST,
        ));
    }
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    let mut m = read_manifest(&dir).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    if let Some(d) = body.description {
        m.description = d;
    }
    if let Some(c) = body.category {
        m.category = Some(c);
    }
    if let Some(p) = body.priority {
        m.priority = p;
    }
    if let Some(e) = body.enabled {
        m.enabled = e;
    }
    write_manifest(&dir, &m).map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    let report = state.reload_plugins_and_l7_async().await;
    Ok(Json(json!({ "ok": true, "report": report })))
}

fn apply_content(
    dir: &FsPath,
    m: &mut PluginManifest,
    rules_json: Option<&str>,
    script: Option<&str>,
) -> Result<(), String> {
    let ptype = m.plugin_type.to_lowercase();
    if ptype == "json" {
        if let Some(raw) = rules_json {
            write_rules_json(dir, raw)?;
            m.rules = vec!["rules/*.json".into()];
        }
    } else if ptype == "lua" {
        if let Some(content) = script {
            let script_file = m.script.clone().unwrap_or_else(|| "plugin.lua".into());
            std::fs::write(dir.join(&script_file), content).map_err(|e| e.to_string())?;
            m.script = Some(script_file);
        }
    }
    Ok(())
}

async fn publish_l7_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<PublishBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    let mut m = read_manifest(&dir).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let old_version = m.version.clone();
    let note = body.note.clone().unwrap_or_default();

    // Lazy archive: ensure older versions enter history.
    archive_current(&dir, &old_version, "auto-archive")
        .map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    let new_version = bump_patch(&old_version);
    if let Some(d) = body.description {
        m.description = d;
    }
    if let Some(c) = body.category {
        m.category = Some(c);
    }
    if let Some(p) = body.priority {
        m.priority = p;
    }
    apply_content(
        &dir,
        &mut m,
        body.rules_json.as_deref(),
        body.script.as_deref(),
    )
    .map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    m.version = new_version.clone();
    write_manifest(&dir, &m).map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;
    archive_current(&dir, &new_version, &note)
        .map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?;

    let report = state.reload_plugins_and_l7_async().await;
    Ok(Json(json!({
        "ok": true,
        "version": new_version,
        "previousVersion": old_version,
        "report": report
    })))
}

async fn list_l7_versions(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    let current = read_manifest(&dir).map(|m| m.version).unwrap_or_default();
    let vroot = versions_root(&dir);
    let mut versions = Vec::new();
    if vroot.is_dir() {
        if let Ok(rd) = std::fs::read_dir(&vroot) {
            for e in rd.filter_map(|x| x.ok()) {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                let folder = e.file_name().to_string_lossy().to_string();
                if !folder.starts_with('v') {
                    continue;
                }
                let ver = folder.trim_start_matches('v').to_string();
                let meta = read_version_meta(&p);
                versions.push(json!({
                    "version": ver,
                    "publishedAt": meta.published_at,
                    "note": meta.note,
                    "current": ver == current,
                }));
            }
        }
    }
    versions.sort_by(|a, b| {
        let pa = a.get("publishedAt").and_then(|x| x.as_str()).unwrap_or("");
        let pb = b.get("publishedAt").and_then(|x| x.as_str()).unwrap_or("");
        pb.cmp(pa)
    });
    Ok(Json(json!({
        "current": current,
        "versions": versions
    })))
}

async fn get_l7_version(
    State(state): State<Arc<AppState>>,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let ver = validate_version(&version).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    let vdir = versions_root(&dir).join(version_dir_name(&ver));
    if !vdir.is_dir() {
        return Err(map_err_status("版本不存在".into(), StatusCode::NOT_FOUND));
    }
    let mut body = detail_json(&vdir).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let meta = read_version_meta(&vdir);
    body["versionMeta"] = json!(meta);
    body["version"] = Value::String(ver);
    Ok(Json(body))
}

async fn rollback_l7_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<RollbackBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let ver =
        validate_version(&body.version).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    let vdir = versions_root(&dir).join(version_dir_name(&ver));
    if !vdir.is_dir() {
        return Err(map_err_status("版本不存在".into(), StatusCode::NOT_FOUND));
    }

    let hist = read_manifest(&vdir).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let ptype = hist.plugin_type.to_lowercase();
    let rules_json = if ptype == "json" {
        Some(
            load_rules_json_concat(&vdir)
                .map_err(|e| map_err_status(e, StatusCode::INTERNAL_SERVER_ERROR))?,
        )
    } else {
        None
    };
    let script = if ptype == "lua" {
        let script_file = hist.script.clone().unwrap_or_else(|| "plugin.lua".into());
        Some(
            std::fs::read_to_string(vdir.join(&script_file))
                .map_err(|e| map_err_status(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?,
        )
    } else {
        None
    };

    let note = body.note.unwrap_or_else(|| format!("rollback from {ver}"));

    // Reuse the publish path.
    let publish = PublishBody {
        description: Some(hist.description),
        category: hist.category,
        priority: Some(hist.priority),
        note: Some(note),
        rules_json,
        script,
    };
    publish_l7_plugin(State(state), Path(name), Json(publish)).await
}

async fn delete_l7_plugin(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    validate_name(&name).map_err(|e| map_err_status(e, StatusCode::BAD_REQUEST))?;
    let dir = plugin_dir(&plugins_root(&state), &name);
    if !dir.is_dir() {
        return Err(map_err_status("插件不存在".into(), StatusCode::NOT_FOUND));
    }
    std::fs::remove_dir_all(&dir)
        .map_err(|e| map_err_status(format!("删除失败: {e}"), StatusCode::INTERNAL_SERVER_ERROR))?;
    let report = state.reload_plugins_and_l7_async().await;
    Ok(Json(json!({ "ok": true, "report": report })))
}
