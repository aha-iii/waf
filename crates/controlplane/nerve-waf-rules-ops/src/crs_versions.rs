//! CRS 规则集版本管理：快照、回滚、保留最近 N 个版本。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_CRS_VERSIONS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrsVersionRecord {
    pub id: String,
    pub created_at: String,
    #[serde(default)]
    pub note: String,
    pub pack_count: usize,
    pub rule_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs_version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CrsVersionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub fn crs_versions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("rules-versions").join("crs")
}

pub fn list_crs_versions(versions_dir: &Path) -> Result<Vec<CrsVersionRecord>, CrsVersionError> {
    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }
    let index_path = versions_dir.join("index.json");
    if index_path.exists() {
        let raw = fs::read_to_string(&index_path)?;
        return Ok(serde_json::from_str(&raw)?);
    }
    let mut versions = Vec::new();
    for entry in fs::read_dir(versions_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let meta_path = entry.path().join("meta.json");
        if meta_path.exists() {
            if let Ok(raw) = fs::read_to_string(&meta_path) {
                if let Ok(v) = serde_json::from_str::<CrsVersionRecord>(&raw) {
                    versions.push(v);
                }
            }
        }
    }
    versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(versions)
}

pub fn snapshot_crs(
    rules_dir: &Path,
    versions_dir: &Path,
    note: &str,
    crs_version: Option<&str>,
) -> Result<CrsVersionRecord, CrsVersionError> {
    fs::create_dir_all(versions_dir)?;
    let (pack_count, rule_count) = count_rules_in_dir(rules_dir);
    let now = chrono::Utc::now();
    let id = format!("{}", now.format("%Y%m%d-%H%M%S%.3f")).replace('.', "");
    let record = CrsVersionRecord {
        id: id.clone(),
        created_at: now.to_rfc3339(),
        note: note.into(),
        pack_count,
        rule_count,
        crs_version: crs_version.map(str::to_string),
    };

    let version_dir = versions_dir.join(&id);
    fs::create_dir_all(&version_dir)?;
    copy_json_tree(rules_dir, &version_dir)?;
    fs::write(
        version_dir.join("meta.json"),
        serde_json::to_string_pretty(&record)?,
    )?;

    let mut versions = list_crs_versions(versions_dir)?;
    versions.retain(|v| v.id != id);
    versions.insert(0, record.clone());
    prune_versions(versions_dir, &mut versions, MAX_CRS_VERSIONS);
    fs::write(
        versions_dir.join("index.json"),
        serde_json::to_string_pretty(&versions)?,
    )?;
    Ok(record)
}

pub fn rollback_crs(
    rules_dir: &Path,
    versions_dir: &Path,
    version_id: &str,
) -> Result<CrsVersionRecord, CrsVersionError> {
    let version_dir = versions_dir.join(version_id);
    if !version_dir.is_dir() {
        return Err(CrsVersionError::Other(format!("version not found: {version_id}")));
    }
    let meta_path = version_dir.join("meta.json");
    let target: CrsVersionRecord = if meta_path.exists() {
        serde_json::from_str(&fs::read_to_string(&meta_path)?)?
    } else {
        return Err(CrsVersionError::Other(format!("invalid version: {version_id}")));
    };

    snapshot_crs(rules_dir, versions_dir, &format!("回滚前自动备份 → {version_id}"), None)?;

    clear_json_files(rules_dir)?;
    copy_json_tree(&version_dir, rules_dir)?;
    Ok(target)
}

fn prune_versions(versions_dir: &Path, versions: &mut Vec<CrsVersionRecord>, keep: usize) {
    while versions.len() > keep {
        if let Some(old) = versions.pop() {
            let _ = fs::remove_dir_all(versions_dir.join(&old.id));
        }
    }
}

fn count_rules_in_dir(dir: &Path) -> (usize, usize) {
    let mut packs = 0usize;
    let mut rules = 0usize;
    if !dir.is_dir() {
        return (0, 0);
    }
    if let Ok(entries) = glob::glob(&format!("{}/**/*.json", dir.display())) {
        for entry in entries.flatten() {
            if entry.file_name().and_then(|n| n.to_str()) == Some("meta.json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&entry) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if v.get("pack_id").is_some() {
                        packs += 1;
                    }
                    if let Some(arr) = v.get("rules").and_then(|r| r.as_array()) {
                        rules += arr.len();
                    }
                }
            }
        }
    }
    (packs, rules)
}

pub fn replace_rules_dir(from: &Path, to: &Path) -> Result<(), CrsVersionError> {
    fs::create_dir_all(to)?;
    clear_json_files(to)?;
    copy_json_tree(from, to)
}

fn copy_json_tree(from: &Path, to: &Path) -> Result<(), CrsVersionError> {
    if !from.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_json_tree(&path, &to.join(entry.file_name()))?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json")
            && entry.file_name() != "meta.json"
        {
            fs::copy(&path, to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn clear_json_files(dir: &Path) -> Result<(), CrsVersionError> {
    if !dir.is_dir() {
        fs::create_dir_all(dir)?;
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            clear_json_files(&path)?;
            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
