//! Compile WAF-L3 / plugin configs into SyncPlan for Agent eBPF.

use nerve_waf_client::NerveClient;
use nerve_waf_plugins::get_plugin;
use nerve_waf_sdk::{
    LogLevel, L7Bundle, PluginAction, PluginContext, ProtectionModules, SyncPlan, SyncResult,
    WafConfig, WafRoute,
};
use std::collections::HashSet;

pub use nerve_waf_plugins::list_plugins;
pub use nerve_waf_sdk::PluginMeta;

pub fn compile_config(config: &WafConfig, modules: &ProtectionModules, l7_rule_count: usize) -> SyncPlan {
    compile_config_with_runtime(config, modules, l7_rule_count, &[])
}

pub fn compile_config_with_runtime(
    config: &WafConfig,
    modules: &ProtectionModules,
    l7_rule_count: usize,
    runtime_deny_ips: &[String],
) -> SyncPlan {
    let mut plan = compile_config_inner(config, modules, l7_rule_count);
    merge_runtime_denies(&mut plan, runtime_deny_ips);
    plan
}

fn merge_runtime_denies(plan: &mut SyncPlan, runtime_deny_ips: &[String]) {
    let mut existing = HashSet::new();
    for action in &plan.actions {
        if let PluginAction::DenyIp { ip, .. } = action {
            existing.insert(ip.clone());
        }
    }
    for ip in runtime_deny_ips {
        if existing.insert(ip.clone()) {
            plan.actions.push(PluginAction::DenyIp {
                ip: ip.clone(),
                reason: Some("runtime deny overlay".into()),
            });
        }
    }
}

fn compile_config_inner(config: &WafConfig, modules: &ProtectionModules, l7_rule_count: usize) -> SyncPlan {
    let mut actions = Vec::new();
    let mut plugins_applied = 0;

    let mut routes: Vec<&WafRoute> = config.routes.iter().filter(|r| r.enabled).collect();
    routes.sort_by_key(|r| r.priority);

    for route in routes {
        let mut plugin_refs: Vec<_> = route
            .plugins
            .iter()
            .filter(|p| p.enabled)
            .filter_map(|p| get_plugin(&p.name).map(|plugin| (p, plugin)))
            .collect();
        plugin_refs.sort_by_key(|(_, plugin)| plugin.meta().priority);

        for (ref_, plugin) in plugin_refs {
            match plugin.validate_config(&ref_.config) {
                Ok(valid) => {
                    let ctx = PluginContext {
                        route,
                        match_: route.route_match(),
                    };
                    actions.extend(plugin.apply(&ctx, &valid));
                    plugins_applied += 1;
                }
                Err(err) => actions.push(PluginAction::Log {
                    level: LogLevel::Warn,
                    message: format!("插件 {} 配置无效: {err}", ref_.name),
                }),
            }
        }
    }

    SyncPlan {
        actions,
        routes_applied: config.routes.iter().filter(|r| r.enabled).count(),
        plugins_applied,
        l7_bundle: Some(L7Bundle {
            modules: modules.clone(),
            rule_count: l7_rule_count,
        }),
    }
}

pub async fn sync_to_nerve(client: &NerveClient, plan: &SyncPlan) -> SyncResult {
    let body = sync_plan_json(plan);
    if let Ok(resp) = client.sync_plan(&body).await {
        let applied = resp
            .get("applied")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        return SyncResult {
            ok: resp.get("status").and_then(|v| v.as_str()) != Some("partial"),
            plan: plan.clone(),
            applied,
            errors: vec![],
        };
    }

    let mut errors = Vec::new();
    let mut applied = 0;

    for action in &plan.actions {
        let result = match action {
            PluginAction::DenyIp { ip, .. } => client.deny_ip(ip).await,
            PluginAction::AllowIp { ip, .. } => client.allow_ip(ip).await,
            PluginAction::BaselineAllow { pod, ip, .. } => client.baseline_allow(pod, ip).await,
            PluginAction::Log { .. } => Ok(()),
        };

        if let Err(e) = result {
            errors.push(format!("{}: {e}", action_kind(action)));
        } else if !matches!(action, PluginAction::Log { .. }) {
            applied += 1;
        }
    }

    SyncResult {
        ok: errors.is_empty(),
        plan: plan.clone(),
        applied,
        errors,
    }
}

/// Builds Network 2.0 Agent `/api/sync/plan` JSON (Agent BPF + CNI deny + L4 NP).
pub fn sync_plan_json(plan: &SyncPlan) -> serde_json::Value {
    sync_plan_with_l4(
        plan,
        &nerve_waf_np_compiler::CompiledL4Policy::default(),
        &nerve_waf_np_compiler::IdentityMap::default(),
    )
}

/// SyncPlan body with L4 NP + identity map.
pub fn sync_plan_with_l4(
    plan: &SyncPlan,
    l4: &nerve_waf_np_compiler::CompiledL4Policy,
    id_map: &nerve_waf_np_compiler::IdentityMap,
) -> serde_json::Value {
    let mut deny = Vec::new();
    let mut allow = Vec::new();
    let mut baseline: Vec<serde_json::Value> = Vec::new();
    let mut baseline_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for action in &plan.actions {
        match action {
            PluginAction::DenyIp { ip, .. } => deny.push(ip.clone()),
            PluginAction::AllowIp { ip, .. } => allow.push(ip.clone()),
            PluginAction::BaselineAllow { pod, ip, .. } => {
                baseline_map.entry(pod.clone()).or_default().push(ip.clone());
            }
            PluginAction::Log { .. } => {}
        }
    }
    for (pod, ips) in baseline_map {
        baseline.push(serde_json::json!({ "pod": pod, "ips": ips }));
    }

    let mut body = serde_json::json!({
        "version": 2,
        "deny": deny,
        "allow_ips": allow,
        "baseline": baseline,
        "l7_mode": if plan.l7_bundle.is_some() { "block" } else { "observe" },
    });

    if !l4.rules.is_empty() || !l4.endpoints.is_empty() || !l4.lpm_rules.is_empty() {
        body["l4_policy"] = serde_json::json!({
            "version": l4.version,
            "rules": l4.rules,
            "lpm_rules": l4.lpm_rules,
            "endpoints": l4.endpoints,
        });
    }
    if !id_map.records.is_empty() {
        body["identity"] = serde_json::json!({
            "version": id_map.version,
            "records": id_map.records,
        });
    }
    body
}

pub async fn sync_config(
    client: &NerveClient,
    config: &WafConfig,
    modules: &ProtectionModules,
    l7_rule_count: usize,
) -> SyncResult {
    sync_config_with_runtime(client, config, modules, l7_rule_count, &[]).await
}

pub async fn sync_config_with_runtime(
    client: &NerveClient,
    config: &WafConfig,
    modules: &ProtectionModules,
    l7_rule_count: usize,
    runtime_deny_ips: &[String],
) -> SyncResult {
    let plan = compile_config_with_runtime(config, modules, l7_rule_count, runtime_deny_ips);
    sync_to_nerve(client, &plan).await
}

fn action_kind(action: &PluginAction) -> &'static str {
    match action {
        PluginAction::DenyIp { .. } => "deny_ip",
        PluginAction::AllowIp { .. } => "allow_ip",
        PluginAction::BaselineAllow { .. } => "baseline_allow",
        PluginAction::Log { .. } => "log",
    }
}

#[cfg(test)]
mod test;
