use crate::config::{SemanticAutoTune, SemanticConfig, SemanticMode, SemanticWhitelist};
use crate::stats::SemanticFeedback;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutoTuneResult {
    pub actions: Vec<String>,
    pub config_changed: bool,
}

/// Auto-tune allowlist / semantic mode from FP feedback history.
pub fn apply_auto_tune(
    cfg: &mut SemanticConfig,
    tune: &SemanticAutoTune,
    history: &[SemanticFeedback],
    current: &SemanticFeedback,
) -> AutoTuneResult {
    let mut result = AutoTuneResult::default();
    if !tune.enabled || current.verdict != "false_positive" {
        return result;
    }

    let window_secs = u64::from(tune.window_hours) * 3600;
    let now = current.ts.parse::<u64>().unwrap_or(0);
    let in_window = |fb: &SemanticFeedback| {
        let ts = fb.ts.parse::<u64>().unwrap_or(0);
        now.saturating_sub(ts) <= window_secs
    };

    let fp_history: Vec<_> = history
        .iter()
        .filter(|fb| fb.verdict == "false_positive" && in_window(fb))
        .collect();

    if tune.auto_whitelist_param {
        if let Some(param_path) = current.param_path.as_deref().filter(|s| !s.is_empty()) {
            let same_param = fp_history
                .iter()
                .filter(|fb| fb.param_path.as_deref() == Some(param_path))
                .count() as u32
                + 1;
            if same_param >= tune.false_positive_threshold {
                if let Some(name) = param_name_from_path(param_path) {
                    if add_whitelist_param(&mut cfg.whitelist, &name) {
                        result.actions.push(format!("whitelist_param:{name}"));
                        result.config_changed = true;
                    }
                }
            }
        }
    }

    if tune.auto_relax_mode && cfg.mode != SemanticMode::Relaxed {
        let total_fp = fp_history.len() as u32 + 1;
        if total_fp >= tune.relax_after_fp_count {
            cfg.mode = SemanticMode::Relaxed;
            result.actions.push("mode:relaxed".into());
            result.config_changed = true;
        }
    }

    result
}

fn param_name_from_path(param_path: &str) -> Option<String> {
    if param_path == "path" || param_path == "body" {
        return None;
    }
    let rest = param_path
        .strip_prefix("query.")
        .or_else(|| param_path.strip_prefix("body."))
        .unwrap_or(param_path);
    let name = rest.split('.').next()?.split('@').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn add_whitelist_param(wl: &mut SemanticWhitelist, name: &str) -> bool {
    if wl.param_names.iter().any(|n| n == name) {
        return false;
    }
    wl.param_names.push(name.to_string());
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::SemanticFeedback;

    fn fp(param_path: &str, ts: u64) -> SemanticFeedback {
        SemanticFeedback {
            ts: ts.to_string(),
            site_id: "default".into(),
            event_id: None,
            verdict: "false_positive".into(),
            note: None,
            param_path: Some(param_path.into()),
            uri: None,
            applied_action: None,
        }
    }

    #[test]
    fn auto_whitelist_after_threshold() {
        let mut cfg = SemanticConfig::default();
        let tune = SemanticAutoTune {
            false_positive_threshold: 2,
            ..Default::default()
        };
        let history = vec![fp("query.token", 100)];
        let current = fp("query.token", 200);
        let r = apply_auto_tune(&mut cfg, &tune, &history, &current);
        assert!(r.config_changed);
        assert!(cfg.whitelist.param_names.contains(&"token".to_string()));
    }
}
