use glob::glob;
use nerve_waf_l7::{compile_rules_from_json, CompiledRule};
use std::path::{Path, PathBuf};

pub fn load_rules_from_manifest(
    dir: &Path,
    patterns: &[String],
) -> Result<Vec<CompiledRule>, String> {
    let mut rules = Vec::new();
    let default_patterns = vec!["rules/*.json".to_string()];
    let use_patterns = if patterns.is_empty() {
        &default_patterns
    } else {
        patterns
    };
    for pattern in use_patterns {
        let pat = dir.join(pattern.replace('/', std::path::MAIN_SEPARATOR_STR));
        let pat_str = pat.to_string_lossy().replace('\\', "/");
        for entry in glob(&pat_str).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?;
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| format!("read {}: {e}", path.display()))?;
            let source = path.to_string_lossy().to_string();
            let compiled =
                compile_rules_from_json(&raw, &source).map_err(|e| format!("{source}: {e}"))?;
            rules.extend(compiled);
        }
    }
    Ok(rules)
}

pub fn discover_manifest_dirs(plugins_dir: &Path, layer: &str) -> Vec<PathBuf> {
    let layer_dir = plugins_dir.join(layer);
    let mut dirs = Vec::new();
    if !layer_dir.is_dir() {
        return dirs;
    }
    if let Ok(entries) = std::fs::read_dir(&layer_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").is_file() {
                dirs.push(path);
            }
        }
    }
    dirs.sort();
    dirs
}
