//! Load/merge CRS and custom rule packs with rollout helpers.

mod crs_versions;

pub use crs_versions::{
    crs_versions_dir, list_crs_versions, replace_rules_dir, rollback_crs, snapshot_crs,
    CrsVersionError, CrsVersionRecord, MAX_CRS_VERSIONS,
};

use nerve_waf_l7::{compile_rules_from_json, CompiledRule, RawRule, RuleCompileError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesPackMeta {
    pub pack_id: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_rollout")]
    pub rollout_pct: u8,
    #[serde(default)]
    pub enabled: bool,
}

fn default_rollout() -> u8 {
    100
}

#[derive(Debug, Clone, Deserialize)]
struct PackFile {
    #[serde(flatten)]
    meta: RulesPackMeta,
    rules: Vec<RawRule>,
}

#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub meta: RulesPackMeta,
    pub rules: Vec<CompiledRule>,
}

#[derive(Debug, thiserror::Error)]
pub enum RulesOpsError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("compile: {0}")]
    Compile(#[from] RuleCompileError),
    #[error("invalid pack: {0}")]
    Invalid(String),
}

/// Load all `*.json` rule packs from a directory (skip sample when disabled).
pub fn load_packs_from_dir(dir: &Path) -> Result<Vec<LoadedPack>, RulesOpsError> {
    let mut packs = Vec::new();
    if !dir.is_dir() {
        return Ok(packs);
    }
    for entry in glob::glob(&format!("{}/**/*.json", dir.display())).unwrap_or_else(|_| glob::glob("").unwrap()) {
        let path = entry.map_err(|e| RulesOpsError::Invalid(e.to_string()))?;
        if let Ok(pack) = load_pack_file(&path) {
            packs.push(pack);
        }
    }
    Ok(packs)
}

/// Load RawRule from directory (no compile; for Edge dataplane snapshot serialization).
pub fn load_raw_rules_from_dir(dir: &Path) -> Result<Vec<RawRule>, RulesOpsError> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in glob::glob(&format!("{}/**/*.json", dir.display())).unwrap_or_else(|_| glob::glob("").unwrap()) {
        let path = entry.map_err(|e| RulesOpsError::Invalid(e.to_string()))?;
        let raw = std::fs::read_to_string(&path)?;
        let file: PackFile = serde_json::from_str(&raw).map_err(|e| RulesOpsError::Invalid(e.to_string()))?;
        if !file.meta.enabled || file.meta.rollout_pct == 0 {
            continue;
        }
        out.extend(file.rules);
    }
    Ok(out)
}

pub fn load_pack_file(path: &Path) -> Result<LoadedPack, RulesOpsError> {
    let raw = std::fs::read_to_string(path)?;
    let file: PackFile = serde_json::from_str(&raw).map_err(|e| RulesOpsError::Invalid(e.to_string()))?;
    if !file.meta.enabled && file.meta.rollout_pct == 0 {
        return Ok(LoadedPack {
            meta: file.meta,
            rules: vec![],
        });
    }
    let source = path.to_string_lossy().to_string();
    let rules = compile_rules_from_json(
        &serde_json::to_string(&file.rules).unwrap_or_else(|_| "[]".into()),
        &source,
    )?;
    Ok(LoadedPack {
        meta: file.meta,
        rules,
    })
}

/// Merge embedded + external packs; rollout_pct trims per pack (simplified: full or 0).
pub fn merge_rules(base: Vec<CompiledRule>, packs: &[LoadedPack]) -> Vec<CompiledRule> {
    let mut out = base;
    for pack in packs {
        if !pack.meta.enabled {
            continue;
        }
        if pack.meta.rollout_pct == 0 {
            continue;
        }
        out.extend(pack.rules.clone());
    }
    out
}

/// Merge rules; patches with same id override packs.
pub fn merge_rules_with_patches(
    base: Vec<CompiledRule>,
    packs: &[LoadedPack],
    patches: &[LoadedPack],
) -> Vec<CompiledRule> {
    use std::collections::HashMap;
    let mut by_id: HashMap<String, CompiledRule> = HashMap::new();
    for r in base {
        by_id.insert(r.id.clone(), r);
    }
    for pack in packs {
        if !pack.meta.enabled || pack.meta.rollout_pct == 0 {
            continue;
        }
        for r in &pack.rules {
            by_id.insert(r.id.clone(), r.clone());
        }
    }
    for patch in patches {
        if !patch.meta.enabled {
            continue;
        }
        for r in &patch.rules {
            by_id.insert(r.id.clone(), r.clone());
        }
    }
    by_id.into_values().collect()
}

pub fn list_pack_meta(packs: &[LoadedPack]) -> Vec<RulesPackMeta> {
    packs.iter().map(|p| p.meta.clone()).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct PackSummary {
    #[serde(flatten)]
    pub meta: RulesPackMeta,
    pub rule_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub severity: String,
    pub paranoia_level: u8,
    pub pattern: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RulesInventory {
    pub rules_dir: String,
    pub embedded_rules: usize,
    pub total_rules: usize,
    pub packs: Vec<PackSummary>,
    pub patches: Vec<PackSummary>,
    pub ai_packs: Vec<PackSummary>,
    pub rules: Vec<RuleSummary>,
    pub rules_total: usize,
    pub rules_truncated: bool,
}

pub fn summarize_packs(packs: &[LoadedPack]) -> Vec<PackSummary> {
    packs
        .iter()
        .map(|p| PackSummary {
            meta: p.meta.clone(),
            rule_count: p.rules.len(),
        })
        .collect()
}

pub fn summarize_rules(rules: &[CompiledRule], limit: usize) -> (Vec<RuleSummary>, usize) {
    let total = rules.len();
    let out = rules
        .iter()
        .take(limit)
        .map(|r| RuleSummary {
            id: r.id.clone(),
            name: r.name.clone(),
            category: r.category.clone(),
            severity: r.severity.clone(),
            paranoia_level: r.paranoia_level,
            pattern: r.pattern.clone(),
            source: r.source.clone(),
        })
        .collect();
    (out, total)
}
