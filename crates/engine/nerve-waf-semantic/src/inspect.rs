use crate::config::{SemanticConfig, SemanticMode, SemanticWhitelist};
use crate::intent::{classify_sql_intent, intent_label, SqlIntent};
use crate::stats::{now_iso, InspectTimer, SemanticSample, SemanticStatsHandle};
use libinjectionrs::{detect_sqli, detect_xss};
use nerve_waf_libinjection::{
    effective_rule_enabled, libinjection_rule_catalog, LibInjectionConfig, LibInjectionRuleDef,
    ScanTarget,
};
use nerve_waf_l7::{InspectRequest, InspectResult};
use nerve_waf_normalize::{
    decode_chain, is_static_asset_path, locate_params_scoped, looks_suspicious, DecodeLayer,
    ParamKind, ParamSlice,
};
use nerve_waf_sdk::ProtectionModules;

struct CandidateHit {
    category: String,
    rule_id: String,
    rule_name: String,
    severity: String,
    param_path: String,
    layer: DecodeLayer,
    decode_chain: Vec<String>,
    engines: Vec<String>,
    semantic_score: u8,
    sample: String,
}

pub fn inspect_semantic(
    sem_cfg: &SemanticConfig,
    li_cfg: &LibInjectionConfig,
    modules: &ProtectionModules,
    req: &InspectRequest,
    stats: Option<&SemanticStatsHandle<'_>>,
) -> Option<InspectResult> {
    if let Some(s) = stats {
        s.record_inspect();
    }

    if !sem_cfg.enabled || !li_cfg.enabled {
        return None;
    }

    if is_whitelisted_request(req, &sem_cfg.whitelist) {
        if let Some(s) = stats {
            s.record_whitelist_skip();
        }
        return None;
    }

    let skip_path_only = is_static_asset_path(&req.uri)
        && !req.uri.contains('?')
        && req.body.is_empty();

    if skip_path_only {
        return None;
    }

    let mut slices = locate_params_scoped(req, &sem_cfg.scope);
    if slices.len() > sem_cfg.performance.max_slices_per_request {
        slices.truncate(sem_cfg.performance.max_slices_per_request);
    }
    if slices.is_empty() {
        return None;
    }

    let timer = InspectTimer::new(sem_cfg.performance.timeout_ms);
    let mut best: Option<CandidateHit> = None;

    for slice in slices {
        if timer.expired() {
            if let Some(s) = stats {
                s.record_timeout();
            }
            break;
        }
        if is_whitelisted_slice(req, &sem_cfg.whitelist, &slice) {
            if let Some(s) = stats {
                s.record_whitelist_skip();
            }
            continue;
        }
        if slice.raw.len() > sem_cfg.decode.max_expand_bytes {
            continue;
        }

        let layers = decode_chain(&slice.raw, &sem_cfg.decode);
        let chain_labels: Vec<String> = layers
            .iter()
            .map(|l| format!("{}:{}", l.op, l.depth))
            .collect();

        for layer in &layers {
            if timer.expired() {
                if let Some(s) = stats {
                    s.record_timeout();
                }
                break;
            }
            if layer.text.is_empty() {
                continue;
            }
            if sem_cfg.mode.require_suspicious_on_path()
                && !looks_suspicious(&layer.text)
                && layer.depth == 0
                && slice.kind == ParamKind::Path
            {
                continue;
            }

            if sem_cfg.engines.libinjection {
                if let Some(hit) = check_libinjection(
                    li_cfg,
                    modules,
                    sem_cfg.mode,
                    &slice,
                    layer,
                    &chain_labels,
                ) {
                    merge_best(&mut best, hit);
                }
            }

            if sem_cfg.sql_intent_enabled() && looks_sqlish(&layer.text) {
                let run_slow = should_run_slow_path(req, sem_cfg.performance.slow_path_sample_rate);
                if let Some(s) = stats {
                    s.record_slow_path(run_slow);
                }
                if run_slow {
                    if let Some(hit) = check_sql_intent(
                        modules,
                        sem_cfg.mode,
                        &slice,
                        layer,
                        &chain_labels,
                    ) {
                        merge_best(&mut best, hit);
                    }
                }
            }
        }
    }

    if let Some(hit) = best {
        if let Some(s) = stats {
            s.record_hit(SemanticSample {
                ts: now_iso(),
                param_path: hit.param_path.clone(),
                decode_chain: hit.decode_chain.clone(),
                semantic_score: hit.semantic_score,
                engines: hit.engines.clone(),
                uri: req.uri.clone(),
                hit: true,
            });
        }
        Some(hit.into_result(modules))
    } else {
        if let Some(s) = stats {
            s.record_miss(None);
        }
        None
    }
}

fn is_whitelisted_request(req: &InspectRequest, wl: &SemanticWhitelist) -> bool {
    if wl.path_prefixes.is_empty() {
        return false;
    }
    let path = req.uri.split('?').next().unwrap_or(&req.uri);
    wl.path_prefixes
        .iter()
        .any(|p| !p.is_empty() && path.starts_with(p))
}

fn is_whitelisted_slice(req: &InspectRequest, wl: &SemanticWhitelist, slice: &ParamSlice) -> bool {
    if !wl.param_names.is_empty() {
        for name in &wl.param_names {
            if name.is_empty() {
                continue;
            }
            if slice.path == format!("query.{name}")
                || slice.path == format!("body.{name}")
                || slice.path.ends_with(&format!(".{name}"))
            {
                return true;
            }
        }
    }

    if !wl.content_types.is_empty() && is_body_slice(slice) {
        let ct = content_type(req);
        if wl.content_types.iter().any(|prefix| {
            !prefix.is_empty() && ct.starts_with(&prefix.to_ascii_lowercase())
        }) {
            return true;
        }
    }

    false
}

fn is_body_slice(slice: &ParamSlice) -> bool {
    matches!(
        slice.kind,
        ParamKind::BodyForm | ParamKind::BodyJson | ParamKind::Raw
    )
}

fn content_type(req: &InspectRequest) -> String {
    for line in req.headers.lines() {
        if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-type") {
                return val.split(';').next().unwrap_or(val).trim().to_ascii_lowercase();
            }
        }
    }
    String::new()
}

fn should_run_slow_path(req: &InspectRequest, sample_rate: u8) -> bool {
    if sample_rate >= 100 {
        return true;
    }
    if sample_rate == 0 {
        return false;
    }
    let hash = req.uri.bytes().chain(req.body.bytes()).fold(0u64, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u64::from(b))
    });
    (hash % 100) < u64::from(sample_rate)
}

fn check_libinjection(
    li_cfg: &LibInjectionConfig,
    modules: &ProtectionModules,
    mode: SemanticMode,
    slice: &ParamSlice,
    layer: &DecodeLayer,
    chain: &[String],
) -> Option<CandidateHit> {
    let bytes = layer.text.as_bytes();
    let base_score = mode.libinjection_base_score();
    for rule in libinjection_rule_catalog() {
        let paranoia_max = modules
            .modules
            .get(rule.category)
            .map(|m| nerve_waf_sdk::sensitivity_paranoia_max(&m.sensitivity))
            .unwrap_or(li_cfg.paranoia_max);
        if !effective_rule_enabled(li_cfg, rule, paranoia_max) {
            continue;
        }
        if !module_enabled(modules, rule.category) {
            continue;
        }
        // Same as plain libinjection: honor this app's disabled_rules (e.g. disable 941100).
        if modules.is_rule_disabled(rule.category, rule.id) {
            continue;
        }
        // Same as plain libinjection: filter by CRS scan targets (avoid 941101 scanning query after 941100 disabled).
        if !rule_applies_to_slice(rule, slice) {
            continue;
        }
        let is_hit = if rule.detect_xss {
            detect_xss(bytes).is_injection()
        } else {
            detect_sqli(bytes).is_injection()
        };
        if !is_hit {
            continue;
        }
        return Some(CandidateHit {
            category: rule.category.into(),
            rule_id: rule.id.into(),
            rule_name: rule.name.into(),
            severity: rule.severity.into(),
            param_path: slice.path.clone(),
            layer: layer.clone(),
            decode_chain: chain.to_vec(),
            engines: vec!["libinjection".into()],
            semantic_score: base_score,
            sample: truncate(&layer.text, 200),
        });
    }
    None
}

/// Align semantic ParamKind to libinjection ScanTarget so path rules do not scan query/body.
fn rule_applies_to_slice(rule: &LibInjectionRuleDef, slice: &ParamSlice) -> bool {
    use ParamKind::*;
    use ScanTarget::*;
    let targets = rule.scan_targets;
    match slice.kind {
        Query => targets.iter().any(|t| matches!(t, Args | ArgsNames)),
        BodyForm | BodyJson | Raw => targets.iter().any(|t| matches!(t, Body | Args | ArgsNames)),
        Cookie => targets.iter().any(|t| matches!(t, Cookies | CookieNames)),
        Path => targets.iter().any(|t| matches!(t, RequestFilename | RequestBasename)),
        Header => {
            let p = slice.path.to_ascii_lowercase();
            if p.contains("user-agent") || p.contains("user_agent") {
                targets.iter().any(|t| matches!(t, UserAgent))
            } else if p.contains("referer") {
                targets.iter().any(|t| matches!(t, Referer))
            } else {
                false
            }
        }
    }
}

fn check_sql_intent(
    modules: &ProtectionModules,
    mode: SemanticMode,
    slice: &ParamSlice,
    layer: &DecodeLayer,
    chain: &[String],
) -> Option<CandidateHit> {
    if !module_enabled(modules, "sqli") {
        return None;
    }
    let (intent, score) = classify_sql_intent(&layer.text);
    let min_score = mode.intent_min_score();
    if score < min_score {
        return None;
    }
    if matches!(intent, SqlIntent::Benign) {
        return None;
    }
    Some(CandidateHit {
        category: "sqli".into(),
        rule_id: "semantic-sqli-intent".into(),
        rule_name: format!("Semantic SQL intent ({})", intent_label(intent)),
        severity: if score >= 80 { "high" } else { "medium" }.into(),
        param_path: slice.path.clone(),
        layer: layer.clone(),
        decode_chain: chain.to_vec(),
        engines: vec!["sqlparser".into(), "intent".into()],
        semantic_score: score,
        sample: truncate(&layer.text, 200),
    })
}

fn looks_sqlish(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.contains("select")
        || lower.contains("union")
        || lower.contains("insert")
        || lower.contains("update")
        || lower.contains("delete")
        || lower.contains(" or ")
}

fn module_enabled(modules: &ProtectionModules, category: &str) -> bool {
    modules
        .modules
        .get(category)
        .map(|m| m.enabled)
        .unwrap_or(matches!(category, "sqli" | "xss"))
}

fn merge_best(best: &mut Option<CandidateHit>, hit: CandidateHit) {
    match best {
        None => *best = Some(hit),
        Some(b) if hit.semantic_score > b.semantic_score => *best = Some(hit),
        _ => {}
    }
}

impl CandidateHit {
    fn into_result(self, modules: &ProtectionModules) -> InspectResult {
        let action = resolve_action(modules, &self.category);
        let blocked = action == "block";
        InspectResult {
            blocked,
            action,
            category: self.category,
            rule_id: self.rule_id,
            rule_name: self.rule_name,
            severity: self.severity,
            matched_field: format!(
                "{}@{}({})",
                self.param_path, self.layer.op, self.layer.depth
            ),
            payload_sample: self.sample,
            plugin_name: Some("nerve-semantic".into()),
            plugin_type: Some("rust".into()),
            param_path: Some(self.param_path),
            decode_chain: Some(self.decode_chain),
            semantic_score: Some(self.semantic_score),
            engines: Some(self.engines),
        }
    }
}

fn resolve_action(modules: &ProtectionModules, category: &str) -> String {
    let global = modules.mode.clone();
    let module_action = modules
        .modules
        .get(category)
        .map(|m| m.action.clone())
        .unwrap_or_else(|| global.clone());
    if module_action == "off" {
        return "allow".into();
    }
    if global == "detect" {
        return "log".into();
    }
    module_action
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nerve_waf_sdk::ProtectionModules;

    #[test]
    fn detects_double_encoded_sqli() {
        let cfg = SemanticConfig::default();
        let li = LibInjectionConfig::default();
        let modules = ProtectionModules::default();
        let hit = inspect_semantic(
            &cfg,
            &li,
            &modules,
            &InspectRequest {
                uri: "/?id=%2527%2520OR%25201%253D1".into(),
                ..Default::default()
            },
            None,
        );
        assert!(hit.is_some(), "expected semantic hit on double-encoded sqli");
    }

    #[test]
    fn path_prefix_whitelist_skips() {
        let mut cfg = SemanticConfig::default();
        cfg.whitelist.path_prefixes = vec!["/static".into()];
        let li = LibInjectionConfig::default();
        let modules = ProtectionModules::default();
        let hit = inspect_semantic(
            &cfg,
            &li,
            &modules,
            &InspectRequest {
                uri: "/static/?id=%2527%2520OR%25201%253D1".into(),
                ..Default::default()
            },
            None,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn disabled_xss_rule_941100_is_skipped() {
        let cfg = SemanticConfig::default();
        let li = LibInjectionConfig::default();
        let mut modules = ProtectionModules::default();
        let xss = modules.modules.entry("xss".into()).or_default();
        xss.enabled = true;
        xss.action = "block".into();
        xss.disabled_rules = vec!["941100".into()];
        let hit = inspect_semantic(
            &cfg,
            &li,
            &modules,
            &InspectRequest {
                uri: "/x?q=<script>alert(1)</script>".into(),
                ..Default::default()
            },
            None,
        );
        assert!(
            hit.is_none(),
            "disabled 941100 must not fire on query XSS (and 941101 must not scan query), got {hit:?}"
        );
    }
}
