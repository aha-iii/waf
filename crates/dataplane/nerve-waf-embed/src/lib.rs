//! 进程内嵌 WAF：同进程调用 `InspectPipeline`，无 HTTP / Unix 跳转。
//!
//! 供 Nerve Gateway `mode=edge` 与 `nerve-waf-edge` 使用。

use nerve_waf_api_sec::ApiSecConfig;
use nerve_waf_behavior::BehaviorEngine;
use nerve_waf_bot::BotConfig;
use nerve_waf_cc::CcEngine;
use nerve_waf_core::{InspectVerdict, WafAction, WafTransaction};
use nerve_waf_dataplane::{GlobalInspectOverlay, SiteInspectSnapshot};
use nerve_waf_geo::GeoEngine;
use nerve_waf_inspect::InspectPipeline;
#[cfg(feature = "inspect-trace")]
use nerve_waf_inspect::{
    finish_and_maybe_log, init_from_env, InspectTraceCtx, InspectTraceMeta,
};
use nerve_waf_intel::IntelFeed;
use nerve_waf_libinjection::LibInjectionConfig;
use nerve_waf_l7::{compile_rules, embedded_rules, L7Engine};
use nerve_waf_plugin_host::{HostedL7, PluginHost};
use nerve_waf_sdk::ProtectionModules;
use nerve_waf_semantic::SemanticConfig;
use nerve_waf_whitelist::WhitelistConfig;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

mod cache;
pub use cache::{new_shared_inspect_cache, SharedInspectCache, SiteInspectCache};
pub use nerve_waf_core::InspectTimingReport;

#[derive(Clone)]
pub struct EmbedInspectRequest {
    pub method: String,
    pub host: String,
    pub uri: String,
    pub headers: String,
    pub body: String,
    pub src_ip: String,
    pub user_agent: String,
    pub response_body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmbedInspectPolicy {
    pub modules: ProtectionModules,
    pub semantic: SemanticConfig,
}

/// 预编译站点 CRS + 模块（edge 按 snapshot generation 缓存，避免每请求 compile_rules）。
pub struct PreparedSiteInspect {
    pub site_l7: L7Engine,
    pub site: SiteInspectSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbedInspectResponse {
    pub allowed: bool,
    pub blocked: bool,
    pub action: String,
    pub rule_name: Option<String>,
    pub rule_id: Option<String>,
    pub category: Option<String>,
    pub severity: Option<String>,
    pub payload_sample: Option<String>,
    pub param_path: Option<String>,
    pub protection_stage: Option<String>,
    pub rule_set: Option<String>,
    pub plugin_name: Option<String>,
    /// Per-node inspect timing（仅 `NERVE_INSPECT_TRACE=1` 时填充）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<nerve_waf_core::InspectTimingReport>,
}

struct GlobalRuntime {
    cc: CcEngine,
    geo: GeoEngine,
    behavior: BehaviorEngine,
    intel: IntelFeed,
    whitelist: WhitelistConfig,
    bot: BotConfig,
    libinjection: LibInjectionConfig,
    api_sec: ApiSecConfig,
}

impl GlobalRuntime {
    fn from_overlay(overlay: &GlobalInspectOverlay) -> Self {
        Self {
            cc: CcEngine::new(overlay.cc.clone()),
            geo: GeoEngine::with_builtin(overlay.geo.clone()),
            behavior: BehaviorEngine::new(overlay.behavior.clone()),
            intel: IntelFeed::from_entries(overlay.intel_entries.clone()),
            whitelist: overlay.whitelist.clone(),
            bot: overlay.bot.clone(),
            libinjection: overlay.libinjection.clone(),
            api_sec: overlay.api_sec.clone(),
        }
    }
}

struct EmbeddedInner {
    modules: ProtectionModules,
    hosted: HostedL7,
    global: GlobalRuntime,
    plugins_dir: PathBuf,
}

pub struct EmbeddedWaf {
    inner: RwLock<EmbeddedInner>,
}

/// 旁路 / Edge 热路径可选参数。
#[derive(Debug, Clone, Default)]
pub struct InspectPreparedOptions {
    /// Bot 挑战已通过时跳过 bot 模块。
    pub bot_challenge_verified: bool,
    /// 可选 trace id（写入 inspect_node_trace 日志）。
    pub trace_id: Option<String>,
}

impl EmbeddedWaf {
    pub fn new(plugins_dir: Option<PathBuf>) -> Self {
        let modules = ProtectionModules::default();
        let dir = plugins_dir.unwrap_or_else(|| PathBuf::from("/var/empty-nerve-waf-plugins"));
        let host = PluginHost::new(dir.clone());
        let hosted = HostedL7::from_host(&host, &modules, embedded_rules());
        let global = GlobalRuntime::from_overlay(&GlobalInspectOverlay::default());
        Self {
            inner: RwLock::new(EmbeddedInner {
                modules,
                hosted,
                global,
                plugins_dir: dir,
            }),
        }
    }

    pub fn modules_version(&self) -> u32 {
        self.inner.read().unwrap().modules.version
    }

    pub fn rule_count(&self) -> usize {
        self.inner.read().unwrap().hosted.rule_count()
    }

    pub fn apply_modules(&self, modules: ProtectionModules) {
        let host = PluginHost::new(self.inner.read().unwrap().plugins_dir.clone());
        let hosted = HostedL7::from_host(&host, &modules, embedded_rules());
        let mut g = self.inner.write().unwrap();
        let prev = g.modules.version;
        g.modules = modules;
        g.hosted = hosted;
        tracing::info!(
            prev_version = prev,
            version = g.modules.version,
            rules = g.hosted.rule_count(),
            "nerve-waf-embed: modules hot-reloaded"
        );
    }

    pub fn apply_global_overlay(&self, overlay: GlobalInspectOverlay) {
        self.inner.write().unwrap().global = GlobalRuntime::from_overlay(&overlay);
    }

    pub fn inspect(&self, req: EmbedInspectRequest) -> EmbedInspectResponse {
        self.inspect_with_policy(req, None)
    }

    pub fn inspect_with_policy(
        &self,
        req: EmbedInspectRequest,
        policy: Option<&EmbedInspectPolicy>,
    ) -> EmbedInspectResponse {
        let g = self.inner.read().unwrap();
        let modules = policy.map(|p| &p.modules).unwrap_or(&g.modules);
        let semantic = policy.map(|p| &p.semantic);
        let site_l7 = g.hosted.engine();
        self.run_pipeline(&g, req, modules, semantic, site_l7, &[])
    }

    pub fn prepare_site_inspect(&self, site: &SiteInspectSnapshot) -> PreparedSiteInspect {
        let g = self.inner.read().unwrap();
        let host = PluginHost::new(g.plugins_dir.clone());
        let crs_rules = compile_rules(site.l7_rules.clone(), "edge-snapshot")
            .unwrap_or_else(|e| {
                tracing::warn!("compile edge l7_rules: {e}");
                Vec::new()
            });
        let site_l7 = host.build_merged_engine(&site.modules, crs_rules);
        PreparedSiteInspect {
            site_l7,
            site: site.clone(),
        }
    }

    pub fn inspect_prepared(
        &self,
        req: EmbedInspectRequest,
        prepared: &PreparedSiteInspect,
        global: &GlobalInspectOverlay,
    ) -> EmbedInspectResponse {
        let verdict = self.inspect_prepared_verdict(req, prepared, global, InspectPreparedOptions::default());
        Self::response_from_verdict(verdict)
    }

    pub fn inspect_prepared_verdict(
        &self,
        req: EmbedInspectRequest,
        prepared: &PreparedSiteInspect,
        global: &GlobalInspectOverlay,
        opts: InspectPreparedOptions,
    ) -> InspectVerdict {
        let g = self.inner.read().unwrap();
        Self::run_prepared_pipeline(&g, &req, prepared, global, opts)
    }

    pub fn response_from_verdict(verdict: InspectVerdict) -> EmbedInspectResponse {
        let mut resp = Self::verdict_to_response(verdict.action, verdict.allowed, verdict.hits.first());
        resp.timing = verdict.timing;
        resp
    }

    fn run_prepared_pipeline(
        g: &EmbeddedInner,
        req: &EmbedInspectRequest,
        prepared: &PreparedSiteInspect,
        _global: &GlobalInspectOverlay,
        opts: InspectPreparedOptions,
    ) -> InspectVerdict {
        // 复用 apply_global_overlay / dataplane sync 已构建的全局引擎，避免每请求重建 Cc/Geo/Behavior。
        let global_rt = &g.global;
        let mut bot = prepared
            .site
            .modules
            .merge_bot_config(&global_rt.bot);
        if opts.bot_challenge_verified {
            bot.enabled = false;
        }
        let tx = WafTransaction::new(
            req.method.clone(),
            req.host.clone(),
            req.uri.clone(),
            req.headers.clone(),
            req.body.clone(),
            req.src_ip.clone(),
            req.user_agent.clone(),
        );
        let pipeline = InspectPipeline {
            cc: Some(&global_rt.cc),
            geo: Some(&global_rt.geo),
            intel: Some(&global_rt.intel),
            api_sec: Some(&global_rt.api_sec),
            whitelist: Some(&global_rt.whitelist),
            bot,
            behavior: Some(&global_rt.behavior),
            libinjection: Some(&global_rt.libinjection),
            semantic: Some(&prepared.site.semantic),
            semantic_stats: None,
            modules: &prepared.site.modules,
            hosted: &g.hosted,
            site_l7: Some(&prepared.site_l7),
            response_body: req.response_body.as_deref(),
            custom_rules: &prepared.site.custom_rules,
        };
        Self::inspect_pipeline_with_optional_trace(&pipeline, tx, req, opts.trace_id.as_deref())
    }

    fn inspect_pipeline_with_optional_trace(
        pipeline: &InspectPipeline<'_>,
        tx: WafTransaction,
        req: &EmbedInspectRequest,
        trace_id: Option<&str>,
    ) -> InspectVerdict {
        #[cfg(feature = "inspect-trace")]
        {
            init_from_env();
            let mut trace = InspectTraceCtx::new();
            if !trace.is_active() {
                return pipeline.inspect(tx);
            }
            let mut verdict = pipeline.inspect_with_trace(tx, Some(&mut trace));
            let blocked = !verdict.allowed;
            if let Some(report) = trace.into_report(blocked) {
                let report = finish_and_maybe_log(
                    report,
                    &InspectTraceMeta {
                        method: req.method.clone(),
                        host: req.host.clone(),
                        uri: req.uri.clone(),
                        src_ip: req.src_ip.clone(),
                        trace_id: trace_id.map(|s| s.to_string()),
                    },
                );
                verdict.timing = Some(report);
            }
            return verdict;
        }
        #[cfg(not(feature = "inspect-trace"))]
        {
            let _ = trace_id;
            pipeline.inspect(tx)
        }
    }

    pub fn inspect_with_snapshot(
        &self,
        req: EmbedInspectRequest,
        site: &SiteInspectSnapshot,
        global: &GlobalInspectOverlay,
    ) -> EmbedInspectResponse {
        let prepared = self.prepare_site_inspect(site);
        self.inspect_prepared(req, &prepared, global)
    }

    fn run_pipeline(
        &self,
        g: &EmbeddedInner,
        req: EmbedInspectRequest,
        modules: &ProtectionModules,
        semantic: Option<&SemanticConfig>,
        site_l7: &L7Engine,
        custom_rules: &[nerve_waf_l7::CustomRule],
    ) -> EmbedInspectResponse {
        let bot = modules.merge_bot_config(&g.global.bot);
        let trace_req = EmbedInspectRequest {
            method: req.method.clone(),
            host: req.host.clone(),
            uri: req.uri.clone(),
            headers: req.headers.clone(),
            body: req.body.clone(),
            src_ip: req.src_ip.clone(),
            user_agent: req.user_agent.clone(),
            response_body: req.response_body.clone(),
        };
        let tx = WafTransaction::new(
            req.method,
            req.host,
            req.uri,
            req.headers,
            req.body,
            req.src_ip,
            req.user_agent,
        );
        let pipeline = InspectPipeline {
            cc: Some(&g.global.cc),
            geo: Some(&g.global.geo),
            intel: Some(&g.global.intel),
            api_sec: Some(&g.global.api_sec),
            whitelist: Some(&g.global.whitelist),
            bot,
            behavior: Some(&g.global.behavior),
            libinjection: Some(&g.global.libinjection),
            semantic,
            semantic_stats: None,
            modules,
            hosted: &g.hosted,
            site_l7: Some(site_l7),
            response_body: req.response_body.as_deref(),
            custom_rules,
        };
        let verdict = Self::inspect_pipeline_with_optional_trace(&pipeline, tx, &trace_req, None);
        let mut resp = Self::verdict_to_response(verdict.action, verdict.allowed, verdict.hits.first());
        resp.timing = verdict.timing;
        resp
    }

    fn verdict_to_response(
        action: WafAction,
        allowed: bool,
        hit: Option<&nerve_waf_core::InspectHit>,
    ) -> EmbedInspectResponse {
        let blocked = matches!(action, WafAction::Block | WafAction::Challenge);
        let action_s = match action {
            WafAction::Challenge => "challenge",
            WafAction::Block => "block",
            WafAction::Log => "log",
            WafAction::Allow => "allow",
        };
        EmbedInspectResponse {
            allowed: allowed && !blocked,
            blocked,
            action: action_s.into(),
            rule_name: hit.map(|h| h.rule_name.clone()),
            rule_id: hit.map(|h| h.rule_id.clone()),
            category: hit.map(|h| h.category.clone()),
            severity: hit.map(|h| h.severity.clone()),
            payload_sample: hit.map(|h| h.payload_sample.clone()),
            param_path: hit.and_then(|h| h.param_path.clone()),
            protection_stage: hit.and_then(|h| h.protection_stage.clone()),
            rule_set: hit.and_then(|h| h.rule_set.clone()),
            plugin_name: hit.and_then(|h| h.plugin_name.clone()),
            timing: None,
        }
    }

    pub fn to_json_value(&self, req: EmbedInspectRequest) -> serde_json::Value {
        let r = self.inspect(req);
        serde_json::json!({
            "allowed": r.allowed,
            "blocked": r.blocked,
            "action": r.action,
            "rule_name": r.rule_name,
            "rule_id": r.rule_id,
        })
    }
}

pub fn global_embedded() -> &'static EmbeddedWaf {
    static INST: OnceLock<EmbeddedWaf> = OnceLock::new();
    INST.get_or_init(|| {
        let plugins = std::env::var("NERVE_WAF_PLUGINS_DIR")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        let waf = EmbeddedWaf::new(plugins);
        tracing::info!(
            rules = waf.rule_count(),
            "nerve-waf-embed: in-process InspectPipeline ready"
        );
        waf
    })
}

#[cfg(test)]
mod test;
