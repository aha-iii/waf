//! 加载与 API inspect 对齐的 per-site 检测输入。

use nerve_waf_dataplane::SiteInspectSnapshot;
use nerve_waf_l7::CustomRule;

use crate::AppState;

pub struct SiteInspectInputs {
    pub snapshot: SiteInspectSnapshot,
    pub custom_rules: Vec<CustomRule>,
}

pub async fn load_site_inspect_inputs(
    state: &AppState,
    site_id: &str,
) -> Result<SiteInspectInputs, String> {
    let modules = state
        .config
        .load_modules_for(site_id)
        .await
        .map_err(|e| e.to_string())?;
    let semantic = state.ai_settings.semantic_config_for(site_id);
    let crs_binding = state
        .config
        .load_app_crs_binding(site_id)
        .await
        .ok()
        .flatten();
    let l7_rules = state
        .platform
        .raw_l7_rules_for_binding(crs_binding.as_ref());
    let custom_rules = state
        .custom_rules_store
        .load_active_rules_for(site_id)
        .await
        .unwrap_or_default();
    let (strategy_id, strategy_name) = resolve_strategy(state, site_id).await;

    Ok(SiteInspectInputs {
        snapshot: SiteInspectSnapshot {
            modules,
            semantic,
            crs_binding,
            l7_rules,
            custom_rules: custom_rules.clone(),
            strategy_id,
            strategy_name,
        },
        custom_rules,
    })
}

async fn resolve_strategy(state: &AppState, site_id: &str) -> (Option<String>, Option<String>) {
    let sid = match state.config.get_app_bound_strategy(site_id).await {
        Ok(id) if !id.is_empty() => id,
        _ => return (None, None),
    };
    let name = match state.config.get_protection_strategy(&sid).await {
        Ok(Some(tpl)) => Some(tpl.name),
        _ => None,
    };
    (Some(sid), name)
}

/// 构建全局 inspect overlay（与 execute_inspect_traced 平台段一致）。
pub fn load_global_inspect_overlay(state: &AppState) -> nerve_waf_dataplane::GlobalInspectOverlay {
    nerve_waf_dataplane::GlobalInspectOverlay {
        whitelist: state.platform.whitelist_config(),
        cc: state.platform.cc.config().clone(),
        bot: state.platform.bot_config(),
        libinjection: state.platform.libinjection_config(),
        geo: state
            .platform
            .geo
            .read()
            .ok()
            .map(|g| g.config().clone())
            .unwrap_or_default(),
        intel_entries: state.platform.intel_entries(),
        api_sec: state.platform.api_sec_config(),
        behavior: state.platform.behavior_config(),
    }
}
