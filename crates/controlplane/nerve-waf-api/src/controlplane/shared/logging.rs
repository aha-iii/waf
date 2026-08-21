//! WAF file logging (structured JSON lines).
//!
//! Config: top-level `logging:` in `config.yml`.

use chrono::{Local, Utc};
use nerve_waf_sdk::WafLoggingConfig;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, SystemTime};

static SETTINGS: OnceLock<RwLock<LogSettings>> = OnceLock::new();

#[derive(Clone)]
struct LogSettings {
    cfg: WafLoggingConfig,
    access_path: PathBuf,
    audit_path: PathBuf,
    error_path: PathBuf,
    server_path: PathBuf,
    access_day: String,
    audit_day: String,
    error_day: String,
    server_day: String,
}

pub fn configure(cfg: &WafLoggingConfig) {
    let mut cfg = cfg.clone();
    if let Ok(p) = std::env::var("WAF_ACCESS_LOG") {
        if !p.is_empty() {
            cfg.access_log = p;
        }
    }
    if let Ok(p) = std::env::var("WAF_AUDIT_LOG") {
        if !p.is_empty() {
            cfg.audit_log = p;
        }
    }
    if let Ok(p) = std::env::var("WAF_ERROR_LOG") {
        if !p.is_empty() {
            cfg.error_log = p;
        }
    }
    if let Ok(p) = std::env::var("WAF_SERVER_LOG") {
        if !p.is_empty() {
            cfg.server_log = p;
        }
    }
    let rotate = cfg.rotate.to_lowercase();
    let access_path = resolve_path(&cfg.access_log);
    let audit_path = resolve_path(&cfg.audit_log);
    let error_path = resolve_path(&cfg.error_log);
    let server_path = resolve_path(&cfg.server_log);
    let access_day = active_day_for(&access_path, &rotate);
    let audit_day = active_day_for(&audit_path, &rotate);
    let error_day = active_day_for(&error_path, &rotate);
    let server_day = active_day_for(&server_path, &rotate);
    let settings = LogSettings {
        cfg: cfg.clone(),
        access_path,
        audit_path,
        error_path,
        server_path,
        access_day,
        audit_day,
        error_day,
        server_day,
    };
    if let Some(lock) = SETTINGS.get() {
        *lock.write().unwrap() = settings;
    } else {
        let _ = SETTINGS.set(RwLock::new(settings));
    }
    if let Some(lock) = SETTINGS.get() {
        let s = lock.read().unwrap();
        ensure_parent(&s.access_path);
        ensure_parent(&s.audit_path);
        ensure_parent(&s.error_path);
        ensure_parent(&s.server_path);
    }
}

fn settings() -> LogSettings {
    SETTINGS
        .get()
        .map(|l| l.read().unwrap().clone())
        .unwrap_or_else(|| {
            configure(&WafLoggingConfig::default());
            settings()
        })
}

fn active_day_for(path: &Path, rotate: &str) -> String {
    let today = Local::now().format("%Y-%m-%d").to_string();
    if rotate == "daily" && path.exists() {
        if let Ok(meta) = fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                let mod_day = chrono::DateTime::<Local>::from(modified)
                    .format("%Y-%m-%d")
                    .to_string();
                if mod_day != today {
                    let archived = archive_name(path, &mod_day);
                    let _ = fs::rename(path, &archived);
                }
            }
        }
    }
    today
}

fn resolve_path(raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        let base = std::env::var("NERVE_WAF_DATA").unwrap_or_else(|_| "./data".into());
        PathBuf::from(base)
            .parent()
            .unwrap_or(Path::new("."))
            .join(raw.trim_start_matches("./"))
    }
}

fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
}

fn maybe_rotate(settings: &mut LogSettings, kind: &str) {
    let (path, day_field) = match kind {
        "audit" => (&settings.audit_path, &mut settings.audit_day),
        "error" => (&settings.error_path, &mut settings.error_day),
        "server" => (&settings.server_path, &mut settings.server_day),
        _ => (&settings.access_path, &mut settings.access_day),
    };
    let rotate = settings.cfg.rotate.to_lowercase();
    let today = Local::now().format("%Y-%m-%d").to_string();
    if rotate == "daily" {
        if *day_field != today && path.exists() {
            let archived = archive_name(path, day_field);
            let _ = fs::rename(path, &archived);
        }
        *day_field = today;
    } else if rotate == "size" {
        if let Ok(meta) = fs::metadata(path) {
            let max_bytes = settings.cfg.max_size_mb.saturating_mul(1024 * 1024);
            if meta.len() >= max_bytes {
                rotate_by_size(path, settings.cfg.max_backups);
            }
        }
    }
    purge_old_files(path, settings.cfg.max_age_days, settings.cfg.max_backups);
}

fn archive_name(base: &Path, tag: &str) -> PathBuf {
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("log");
    let ext = base.extension().and_then(|s| s.to_str()).unwrap_or("log");
    base.with_file_name(format!("{stem}.{tag}.{ext}"))
}

fn rotate_by_size(path: &Path, max_backups: u32) {
    let keep = max_backups.max(1) as usize;
    for i in (1..keep).rev() {
        let from = numbered_archive(path, i);
        let to = numbered_archive(path, i + 1);
        if from.exists() {
            let _ = fs::rename(&from, &to);
        }
    }
    if path.exists() {
        let _ = fs::rename(path, numbered_archive(path, 1));
    }
}

fn numbered_archive(path: &Path, n: usize) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("log");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("log");
    path.with_file_name(format!("{stem}.{n}.{ext}"))
}

fn purge_old_files(base: &Path, max_age_days: u32, max_backups: u32) {
    let parent = match base.parent() {
        Some(p) => p,
        None => return,
    };
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.is_empty() {
        return;
    }
    let mut archives: Vec<(PathBuf, SystemTime)> = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for ent in entries.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.starts_with(stem) && ent.path() != base {
                if let Ok(meta) = ent.metadata() {
                    if let Ok(modified) = meta.modified() {
                        archives.push((ent.path(), modified));
                    }
                }
            }
        }
    }
    archives.sort_by_key(|(_, t)| *t);
    let max_age = Duration::from_secs(max_age_days as u64 * 86400);
    let now = SystemTime::now();
    for (path, modified) in &archives {
        if max_age_days > 0 {
            if let Ok(age) = now.duration_since(*modified) {
                if age > max_age {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
    archives.retain(|(p, _)| p.exists());
    if max_backups > 0 && archives.len() > max_backups as usize {
        let drop = archives.len() - max_backups as usize;
        for (path, _) in archives.into_iter().take(drop) {
            let _ = fs::remove_file(path);
        }
    }
}

fn append_line(kind: &str, line: &str) {
    let mut settings = settings();
    maybe_rotate(&mut settings, kind);
    if let Some(lock) = SETTINGS.get() {
        *lock.write().unwrap() = settings.clone();
    }
    let path = match kind {
        "audit" => &settings.audit_path,
        "error" => &settings.error_path,
        "server" => &settings.server_path,
        _ => &settings.access_path,
    };
    ensure_parent(path);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn write_json_at(kind: &str, log_type: &str, fields: Value, level: tracing::Level) {
    let settings = settings();
    if settings.cfg.format != "json" {
        return;
    }
    let mut rec = fields;
    if let Some(obj) = rec.as_object_mut() {
        obj.entry("time").or_insert(json!(Utc::now().to_rfc3339()));
        obj.entry("log_type").or_insert(json!(log_type));
    }
    let line = rec.to_string();
    match kind {
        "error" => tracing::error!(target: "waf.error", %line),
        "audit" => tracing::warn!(target: "waf.audit", %line),
        "server" => match level {
            tracing::Level::DEBUG => tracing::debug!(target: "waf.server", %line),
            _ => tracing::info!(target: "waf.server", %line),
        },
        _ => tracing::info!(target: "waf.access", %line),
    }
    append_line(kind, &line);
}

fn write_json(kind: &str, log_type: &str, fields: Value) {
    write_json_at(kind, log_type, fields, tracing::Level::INFO);
}

pub fn log_inspect(fields: Value) {
    write_json("access", "inspect", fields);
}

pub fn log_audit(fields: Value) {
    write_json("audit", "audit", fields);
}

pub fn log_error(fields: Value) {
    write_json("error", "error", fields);
}

pub fn log_server(fields: Value) {
    write_json("server", "server", fields);
}

/// Background topology/flows traffic at DEBUG; WAF inspect (/api/inspect etc.) at INFO.
pub fn log_server_at(level: tracing::Level, fields: Value) {
    write_json_at("server", "server", fields, level);
}

pub fn server_level_for_path(path: &str) -> tracing::Level {
    if path.starts_with("/api/topology") {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    }
}
