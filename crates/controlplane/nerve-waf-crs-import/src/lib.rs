//! Import OWASP ModSecurity CRS into JSON rule packs.
//!
//! Converts `@rx` rules and ModSecurity scan targets (including `chain` steps);
//! skips operators such as `@detectSQLi` / comparisons and unsupported `TX:` vars.

use nerve_waf_l7::{compile_rules, RawChainStep, RawRule, RuleCompileError, ScanTarget};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPackMeta {
    pub pack_id: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub rollout_pct: u8,
}

#[derive(Debug, Clone, Serialize)]
struct PackOutput {
    #[serde(flatten)]
    meta: ImportPackMeta,
    rules: Vec<RawRule>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ImportStats {
    pub files_scanned: usize,
    pub rules_seen: usize,
    pub rules_imported: usize,
    pub rules_skipped: usize,
    /// Count of rules skipped because ModSecurity `chain` semantics are unimplemented (prod gap metric).
    pub rules_skipped_chain: usize,
    pub compile_failed: usize,
    pub packs_written: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("compile: {0}")]
    Compile(#[from] RuleCompileError),
    #[error("{0}")]
    Other(String),
}

pub struct ImportOptions {
    pub crs_dir: PathBuf,
    pub out_dir: PathBuf,
    pub version: String,
    pub paranoia_max: u8,
    pub dry_run: bool,
    /// Clear existing JSON rule packs under out_dir before import.
    pub clean: bool,
}

const RX_OP: &str = "@rx";

pub fn import_crs(opts: &ImportOptions) -> Result<ImportStats, ImportError> {
    std::fs::create_dir_all(&opts.out_dir)?;
    if opts.clean && !opts.dry_run {
        clear_json_packs(&opts.out_dir)?;
    }
    let pattern = format!("{}/rules/REQUEST-*.conf", opts.crs_dir.display());
    let mut stats = ImportStats::default();
    let id_re = Regex::new(r"(?i)SecRule\s+").unwrap();

    for entry in glob::glob(&pattern).map_err(|e| ImportError::Other(e.to_string()))? {
        let path = entry.map_err(|e| ImportError::Other(e.to_string()))?;
        if !path.is_file() {
            continue;
        }
        stats.files_scanned += 1;
        let raw = std::fs::read_to_string(&path)?;
        let joined = join_continuations(&raw);
        let mut rules = Vec::new();
        let chunks = split_secrule_chunks(&joined);
        let mut i = 0;
        while i < chunks.len() {
            let chunk = &chunks[i];
            if !id_re.is_match(chunk) {
                i += 1;
                continue;
            }
            stats.rules_seen += 1;

            let parsed = if actions_has_chain(chunk) {
                let start = i;
                i += 1;
                while i < chunks.len() {
                    // Next line with id: starts a new rule.
                    if extract_kv(&chunks[i], "id:").is_some() {
                        break;
                    }
                    let cont = actions_has_chain(&chunks[i]);
                    i += 1;
                    if !cont {
                        break;
                    }
                }
                let group = &chunks[start..i];
                match parse_chain_group(group, opts.paranoia_max) {
                    Some(rule) => Some(rule),
                    None => {
                        stats.rules_skipped_chain += 1;
                        None
                    }
                }
            } else {
                i += 1;
                parse_secrule(chunk, opts.paranoia_max)
            };

            match parsed {
                Some(rule) => {
                    if compile_rules(vec![rule.clone()], "validate").is_ok() {
                        rules.push(rule);
                        stats.rules_imported += 1;
                    } else {
                        stats.compile_failed += 1;
                        stats.rules_skipped += 1;
                    }
                }
                None => {
                    stats.rules_skipped += 1;
                }
            }
        }

        if rules.is_empty() {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("crs-pack");
        let pack_id = stem.to_lowercase().replace('_', "-");
        let pack = PackOutput {
            meta: ImportPackMeta {
                pack_id: pack_id.clone(),
                version: opts.version.clone(),
                description: format!(
                    "核心规则集 imported from {} (regex subset)",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
                enabled: true,
                rollout_pct: 100,
            },
            rules,
        };

        if !opts.dry_run {
            let out_path = opts.out_dir.join(format!("{pack_id}.json"));
            let json = serde_json::to_string_pretty(&pack)?;
            std::fs::write(out_path, json)?;
            stats.packs_written += 1;
        }
    }

    Ok(stats)
}

fn join_continuations(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines() {
        let trimmed = line.trim_end();
        if trimmed.ends_with('\\') {
            out.push_str(trimmed.trim_end_matches('\\').trim());
            out.push(' ');
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out
}

fn split_secrule_chunks(joined: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in joined.lines() {
        let t = line.trim();
        if t.starts_with("SecRule ") || t.starts_with("SecRule\t") {
            if !current.is_empty() {
                chunks.push(current.trim().to_string());
            }
            current = t.to_string();
        } else if !current.is_empty() && !t.is_empty() && !t.starts_with('#') {
            current.push(' ');
            current.push_str(t);
        }
    }
    if !current.is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

fn actions_has_chain(chunk: &str) -> bool {
    let lower = chunk.to_lowercase();
    // `chain` token inside action string (avoid false hits on variable names).
    lower.contains(",chain")
        || lower.contains("chain,")
        || lower.contains(",chain\"")
        || lower.contains("'chain'")
        || lower.contains("\"chain\"")
}

fn targets_unsupported(chunk: &str) -> bool {
    extract_scan_target_tokens(chunk).iter().any(|t| {
        let u = t.to_ascii_uppercase();
        u.starts_with("TX:")
            || u == "TX"
            || u.starts_with("IP:")
            || u.starts_with("SESSION:")
            || u.starts_with("USER:")
    })
}

fn is_matched_vars_target(chunk: &str) -> bool {
    extract_scan_target_tokens(chunk).iter().any(|t| {
        let u = t.to_ascii_uppercase();
        u == "MATCHED_VARS"
            || u == "MATCHED_VAR"
            || u.starts_with("MATCHED_VARS:")
            || u.starts_with("MATCHED_VAR:")
    })
}

fn collect_scan_targets(chunk: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut scan_targets = Vec::new();
    for token in extract_scan_target_tokens(chunk) {
        if let Some(t) = ScanTarget::from_crs_token(&token) {
            if seen.insert(t) {
                scan_targets.push(t.label().to_string());
            }
        }
    }
    scan_targets
}

pub(crate) fn parse_chain_group(chunks: &[String], paranoia_max: u8) -> Option<RawRule> {
    if chunks.len() < 2 {
        return None;
    }
    let parent = &chunks[0];
    let lower = parent.to_lowercase();
    if lower.contains("pass,")
        && !lower.contains("block,")
        && !lower.contains("deny,")
        && !lower.contains("severity:")
    {
        return None;
    }
    let pl = extract_paranoia_level(parent);
    if pl > paranoia_max {
        return None;
    }

    let mut chain_steps = Vec::new();
    let mut parent_pattern = None;
    let mut parent_targets = Vec::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        if chunk.contains("@detect") || chunk.contains("@lt ") || chunk.contains("@gt ") {
            return None;
        }
        if targets_unsupported(chunk) {
            return None;
        }
        let rx_pat = extract_rx_pattern(chunk)?;
        let on_matched = is_matched_vars_target(chunk);
        let scan_targets = if on_matched {
            Vec::new()
        } else {
            let t = collect_scan_targets(chunk);
            // Child step is neither MATCHED_VARS nor a mappable target → cannot evaluate safely.
            if idx > 0 && t.is_empty() {
                return None;
            }
            t
        };
        if idx == 0 {
            parent_pattern = Some(rx_pat);
            parent_targets = scan_targets;
        } else {
            chain_steps.push(RawChainStep {
                pattern: rx_pat,
                scan_targets,
                on_matched,
            });
        }
    }

    let id = extract_kv(parent, "id:")?;
    let msg = extract_quoted(parent, "msg:").unwrap_or_else(|| format!("CRS rule {id}"));
    let category = infer_category(parent);
    let severity = infer_severity(parent, &category);

    Some(RawRule {
        id,
        name: msg,
        pattern: parent_pattern?,
        category,
        severity,
        paranoia_level: pl,
        scan_targets: parent_targets,
        chain_steps,
    })
}

pub(crate) fn parse_secrule(chunk: &str, paranoia_max: u8) -> Option<RawRule> {
    let lower = chunk.to_lowercase();
    if lower.contains("pass,") && !lower.contains("block,") && !lower.contains("deny,") {
        // Import only block/deny-class detection rules.
        if !lower.contains("severity:") {
            return None;
        }
    }

    // Block contains chain but was not grouped → incomplete; skip.
    if actions_has_chain(chunk) {
        return None;
    }

    let pl = extract_paranoia_level(chunk);
    if pl > paranoia_max {
        return None;
    }

    if chunk.contains("@detect") || chunk.contains("@lt ") || chunk.contains("@gt ") {
        return None;
    }
    if targets_unsupported(chunk) {
        return None;
    }

    let rx_pat = extract_rx_pattern(chunk)?;
    let id = extract_kv(chunk, "id:")?;
    let msg = extract_quoted(chunk, "msg:").unwrap_or_else(|| format!("CRS rule {id}"));
    let category = infer_category(chunk);
    let severity = infer_severity(chunk, &category);
    let scan_targets = collect_scan_targets(chunk);

    Some(RawRule {
        id,
        name: msg,
        pattern: rx_pat,
        category,
        severity,
        paranoia_level: pl,
        scan_targets,
        chain_steps: vec![],
    })
}

fn extract_scan_target_tokens(chunk: &str) -> Vec<String> {
    let rest = match chunk
        .strip_prefix("SecRule")
        .or_else(|| chunk.strip_prefix("secrule"))
    {
        Some(r) => r.trim_start(),
        None => return Vec::new(),
    };
    // Quoted targets: SecRule "ARGS|REQUEST_COOKIES" "@rx ...".
    let quote = rest.chars().next();
    let inner = if quote == Some('"') || quote == Some('\'') {
        let q = quote.unwrap();
        if q == '"' {
            match parse_quoted(rest) {
                Some((s, _)) => s,
                None => return Vec::new(),
            }
        } else {
            match parse_single_quoted(rest) {
                Some((s, _)) => s,
                None => return Vec::new(),
            }
        }
    } else {
        // Unquoted targets (common in CRS): SecRule REQUEST_HEADERS:Referer "@rx ...".
        let end = rest
            .find(" \"@")
            .or_else(|| rest.find(" '@"))
            .or_else(|| rest.find("\"@"))
            .or_else(|| rest.find("'@"))
            .unwrap_or(rest.len());
        rest[..end].trim().to_string()
    };
    inner
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_regex_quoted(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.first()? != &b'"' {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            // ModSecurity quote layer only unescapes \"; keep \[ \] \$ etc. for Rust regex.
            b'\\' if i + 1 < bytes.len() && bytes[i + 1] == b'"' => {
                out.push('"');
                i += 2;
            }
            b'\\' if i + 1 < bytes.len() => {
                out.push('\\');
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'"' => return Some((out, i + 1)),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

fn parse_regex_single_quoted(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.first()? != &b'\'' {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() && bytes[i + 1] == b'\'' => {
                out.push('\'');
                i += 2;
            }
            b'\\' if i + 1 < bytes.len() => {
                out.push('\\');
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'\'' => return Some((out, i + 1)),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

fn extract_rx_pattern(chunk: &str) -> Option<String> {
    // ModSecurity standard form: SecRule TARGETS "@rx <pattern>" "actions".
    // @rx and regex share one quote pair; not a separate quote after @rx.
    for needle in ["\"@rx ", "\"@rx", "'@rx ", "'@rx"] {
        if let Some(idx) = chunk.find(needle) {
            let quote = needle.chars().next().unwrap();
            let quoted = &chunk[idx..];
            let inner = if quote == '"' {
                parse_regex_quoted(quoted)?.0
            } else {
                parse_regex_single_quoted(quoted)?.0
            };
            let pat = inner
                .strip_prefix("@rx")
                .or_else(|| inner.strip_prefix("@RX"))?
                .trim_start();
            return Some(normalize_crs_pattern(pat));
        }
    }

    // Compat split form: "@rx" "pattern".
    let idx = chunk.find(RX_OP)?;
    let after = &chunk[idx + RX_OP.len()..].trim_start();
    if after.starts_with('"') {
        parse_regex_quoted(after).map(|(s, _)| normalize_crs_pattern(&s))
    } else if after.starts_with('\'') {
        parse_regex_single_quoted(after).map(|(s, _)| normalize_crs_pattern(&s))
    } else {
        None
    }
}

/// Convert common ModSecurity/PCRE forms to Rust `regex`-compilable (else return as-is).
fn normalize_crs_pattern(pat: &str) -> String {
    let mut s = pat.to_string();
    // PCRE `\_` inside char class often means literal `_`; Rust needs no escape for `_`.
    s = s.replace("\\_", "_");
    // Strip PCRE conditionals/recursion unsupported by Rust (whole rule skipped at compile).
    s
}

fn parse_quoted(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.first()? != &b'"' {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'"' => return Some((out, i + 1)),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

fn parse_single_quoted(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.first()? != &b'\'' {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'\'' => return Some((out, i + 1)),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

fn extract_kv(chunk: &str, key: &str) -> Option<String> {
    let pos = chunk.find(key)?;
    let rest = &chunk[pos + key.len()..];
    let end = rest.find([',', ' ', '"']).unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

fn extract_quoted(chunk: &str, key: &str) -> Option<String> {
    let pos = chunk.find(key)?;
    let rest = chunk[pos + key.len()..].trim_start();
    if rest.starts_with('\'') {
        parse_single_quoted(rest).map(|(s, _)| s)
    } else if rest.starts_with('"') {
        parse_quoted(rest).map(|(s, _)| s)
    } else {
        None
    }
}

fn extract_paranoia_level(chunk: &str) -> u8 {
    for token in chunk.split(',') {
        let t = token.trim();
        if let Some(rest) = t.strip_prefix("tag:'paranoia-level/") {
            if let Some(n) = rest.strip_suffix('\'').and_then(|s| s.parse().ok()) {
                return n;
            }
        }
        if let Some(rest) = t.strip_prefix("tag:paranoia-level/") {
            if let Ok(n) = rest.parse() {
                return n;
            }
        }
    }
    1
}

fn infer_category(chunk: &str) -> String {
    static TAG_MAP: &[(&str, &str)] = &[
        ("attack-sqli", "sqli"),
        ("attack-xss", "xss"),
        ("attack-lfi", "path_traversal"),
        ("attack-rfi", "ssrf"),
        ("attack-rce", "rce"),
        ("attack-ssrf", "ssrf"),
        ("attack-fixation", "session"),
        ("attack-session-fixation", "session"),
        ("attack-injection-php", "rce"),
        ("attack-injection-generic", "rce"),
        ("attack-protocol", "protocol"),
        ("attack-disclosure", "info_leak"),
        ("attack-redirect", "open_redirect"),
        ("attack-ldap", "ldap"),
        ("attack-vuln", "cve"),
        ("attack-webshell", "webshell"),
        ("attack-scanner", "scanner"),
        ("attack-reputation", "reputation"),
    ];
    for (tag, cat) in TAG_MAP {
        if chunk.contains(&format!("tag:'OWASP_CRS/{tag}'"))
            || chunk.contains(&format!("tag:'{tag}'"))
            || chunk.contains(&format!("tag:{tag}"))
        {
            return (*cat).into();
        }
    }
    if chunk.contains("APPLICATION-ATTACK-SQLI") {
        return "sqli".into();
    }
    if chunk.contains("XSS") {
        return "xss".into();
    }
    if chunk.contains("RCE") {
        return "rce".into();
    }
    if chunk.contains("LFI") || chunk.contains("RFI") {
        return "path_traversal".into();
    }
    "misc".into()
}

fn infer_severity(chunk: &str, category: &str) -> String {
    if let Some(sev) = extract_kv(chunk, "severity:") {
        let s = sev.to_ascii_lowercase();
        if matches!(s.as_str(), "critical" | "high" | "medium" | "low") {
            return s;
        }
    }
    let pl = extract_paranoia_level(chunk);
    match (category, pl) {
        ("sqli" | "rce" | "webshell", 1) => "critical".into(),
        ("sqli" | "rce" | "webshell", _) => "high".into(),
        ("xss" | "ssrf" | "path_traversal", 1..=2) => "high".into(),
        (_, 1) => "medium".into(),
        (_, 2) => "medium".into(),
        _ => "low".into(),
    }
}

pub fn count_existing_rules(dir: &Path) -> usize {
    let mut ids = HashSet::new();
    if !dir.is_dir() {
        return 0;
    }
    if let Ok(entries) = glob::glob(&format!("{}/**/*.json", dir.display())) {
        for entry in entries.flatten() {
            if let Ok(raw) = std::fs::read_to_string(&entry) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(rules) = v.get("rules").and_then(|r| r.as_array()) {
                        for r in rules {
                            if let Some(id) = r.get("id").and_then(|i| i.as_str()) {
                                ids.insert(id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    ids.len()
}

pub fn merge_pack_manifest(out_dir: &Path) -> Result<HashMap<String, usize>, ImportError> {
    let mut manifest = HashMap::new();
    if !out_dir.is_dir() {
        return Ok(manifest);
    }
    for entry in glob::glob(&format!("{}/**/*.json", out_dir.display()))
        .map_err(|e| ImportError::Other(e.to_string()))?
    {
        let path = entry.map_err(|e| ImportError::Other(e.to_string()))?;
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                let pack_id = v
                    .get("pack_id")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let n = v
                    .get("rules")
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                manifest.insert(pack_id, n);
            }
        }
    }
    Ok(manifest)
}

fn clear_json_packs(dir: &Path) -> Result<(), ImportError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            clear_json_packs(&path)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;
