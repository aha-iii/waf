use nerve_waf_api_sec::ApiSecConfig;
use nerve_waf_behavior::BehaviorEngine;
use nerve_waf_bot::BotConfig;
use nerve_waf_cc::{CcCheckRequest, CcEngine};
use nerve_waf_core::{InspectHit, InspectPhase, InspectVerdict, WafAction, WafTransaction};
use nerve_waf_geo::GeoEngine;
use nerve_waf_intel::IntelFeed;
use nerve_waf_libinjection::{inspect_libinjection, LibInjectionConfig};
use nerve_waf_l7::{InspectRequest, InspectResult};
use nerve_waf_plugin_host::HostedL7;
use nerve_waf_sdk::ProtectionModules;
use nerve_waf_semantic::{inspect_semantic, SemanticConfig, SemanticStatsHandle};
use nerve_waf_whitelist::{is_inspect_bypassed, is_whitelisted, WhitelistConfig, WhitelistRequest};

use crate::trace::{maybe_trace, InspectTraceCtx};

pub fn tx_to_l7_request(tx: &WafTransaction) -> InspectRequest {
    InspectRequest {
        method: tx.method.clone(),
        host: tx.host.clone(),
        uri: tx.uri.clone(),
        headers: tx.headers.clone(),
        body: tx.body.clone(),
        src_ip: tx.src_ip.clone(),
        user_agent: tx.user_agent.clone(),
    }
}

pub fn l7_hit_to_inspect_hit(phase: InspectPhase, hit: &InspectResult) -> InspectHit {
    InspectHit {
        phase,
        action: WafAction::from_l7_action(&hit.action),
        category: hit.category.clone(),
        rule_id: hit.rule_id.clone(),
        rule_name: hit.rule_name.clone(),
        severity: hit.severity.clone(),
        matched_field: hit.matched_field.clone(),
        payload_sample: hit.payload_sample.clone(),
        plugin_name: hit.plugin_name.clone(),
        plugin_type: hit.plugin_type.clone(),
        param_path: hit.param_path.clone(),
        decode_chain: hit.decode_chain.clone(),
        semantic_score: hit.semantic_score,
        engines: hit.engines.clone(),
        protection_stage: None,
        rule_set: None,
    }
}

pub struct InspectPipeline<'a> {
    pub cc: Option<&'a CcEngine>,
    pub geo: Option<&'a GeoEngine>,
    pub intel: Option<&'a IntelFeed>,
    pub api_sec: Option<&'a ApiSecConfig>,
    pub whitelist: Option<&'a WhitelistConfig>,
    pub bot: BotConfig,
    pub behavior: Option<&'a BehaviorEngine>,
    pub libinjection: Option<&'a LibInjectionConfig>,
    pub semantic: Option<&'a SemanticConfig>,
    pub semantic_stats: Option<SemanticStatsHandle<'a>>,
    pub modules: &'a ProtectionModules,
    pub hosted: &'a HostedL7,
    /// Per-app CRS engine (app modules + bound version rules); default uses hosted global singleton.
    pub site_l7: Option<&'a nerve_waf_l7::L7Engine>,
    pub response_body: Option<&'a str>,
    pub custom_rules: &'a [nerve_waf_l7::CustomRule],
}

impl<'a> InspectPipeline<'a> {
    pub fn inspect(&self, tx: WafTransaction) -> InspectVerdict {
        self.inspect_with_trace(tx, None)
    }

    pub fn inspect_with_trace(
        &self,
        mut tx: WafTransaction,
        mut trace: Option<&mut InspectTraceCtx>,
    ) -> InspectVerdict {
        if let Some(wl) = self.whitelist {
            if is_inspect_bypassed(wl, &tx.host, &tx.uri, &tx.src_ip) {
                return InspectVerdict::pass();
            }
        }

        let pipeline_order = self
            .semantic
            .map(|s| s.engines.pipeline_order.clone())
            .unwrap_or_else(|| {
                vec![
                    "pre_access".into(),
                    "expert".into(),
                    "core".into(),
                    "custom".into(),
                ]
            });

        let engines = self.semantic.map(|s| &s.engines);
        let mut req: Option<InspectRequest> = None;
        let mut api_sec_done = false;

        for step in &pipeline_order {
            if engines.is_some_and(|e| !e.is_pipeline_stage_enabled(step)) {
                continue;
            }
            match step.as_str() {
                "pre_access" => {
                    if let Some(hit) = maybe_trace(&mut trace, "defender.pre_access", |trace| {
                        self.run_pre_access(&mut tx, trace)
                    }) {
                        return self.finish(tx, hit);
                    }
                }
                "custom" => {
                    if !api_sec_done {
                        api_sec_done = true;
                        if let Some(hit) =
                            maybe_trace(&mut trace, "defender.custom.api_sec", |trace| {
                                let _ = trace;
                                self.run_api_sec(&mut tx)
                            })
                        {
                            return self.finish(tx, hit);
                        }
                    }
                    let req = req.get_or_insert_with(|| {
                        tx.mark_phase(InspectPhase::RequestUpload);
                        tx.mark_phase(InspectPhase::RequestBody);
                        tx_to_l7_request(&tx)
                    });
                    if let Some(hit) =
                        maybe_trace(&mut trace, "defender.custom.rules", |trace| {
                            let _ = trace;
                            self.run_custom_rules(req)
                        })
                    {
                        return self.finish(tx, with_stage(hit, "custom", "custom"));
                    }
                    if let Some(hit) =
                        maybe_trace(&mut trace, "defender.custom.l7", |trace| {
                            let _ = trace;
                            self.run_hosted_plugin(req)
                        })
                    {
                        return self.finish(tx, with_stage(hit, "custom", "l7-json"));
                    }
                }
                "plugin" => {
                    if !api_sec_done {
                        api_sec_done = true;
                        if let Some(hit) =
                            maybe_trace(&mut trace, "defender.custom.api_sec", |trace| {
                                let _ = trace;
                                self.run_api_sec(&mut tx)
                            })
                        {
                            return self.finish(tx, hit);
                        }
                    }
                    let req = req.get_or_insert_with(|| {
                        tx.mark_phase(InspectPhase::RequestUpload);
                        tx.mark_phase(InspectPhase::RequestBody);
                        tx_to_l7_request(&tx)
                    });
                    if let Some(hit) =
                        maybe_trace(&mut trace, "defender.custom.l7", |trace| {
                            let _ = trace;
                            self.run_hosted_plugin(req)
                        })
                    {
                        return self.finish(tx, with_stage(hit, "custom", "l7-json"));
                    }
                }
                "expert" => {
                    if !api_sec_done {
                        api_sec_done = true;
                        if let Some(hit) =
                            maybe_trace(&mut trace, "defender.custom.api_sec", |trace| {
                                let _ = trace;
                                self.run_api_sec(&mut tx)
                            })
                        {
                            return self.finish(tx, hit);
                        }
                    }
                    let req = req.get_or_insert_with(|| {
                        tx.mark_phase(InspectPhase::RequestUpload);
                        tx.mark_phase(InspectPhase::RequestBody);
                        tx_to_l7_request(&tx)
                    });
                    if let Some(hit) = maybe_trace(&mut trace, "defender.expert", |trace| {
                        self.run_expert_stage(req, trace)
                    }) {
                        return self.finish(tx, hit);
                    }
                }
                "core" => {
                    if !api_sec_done {
                        api_sec_done = true;
                        if let Some(hit) =
                            maybe_trace(&mut trace, "defender.custom.api_sec", |trace| {
                                let _ = trace;
                                self.run_api_sec(&mut tx)
                            })
                        {
                            return self.finish(tx, hit);
                        }
                    }
                    let req = req.get_or_insert_with(|| {
                        tx.mark_phase(InspectPhase::RequestUpload);
                        tx.mark_phase(InspectPhase::RequestBody);
                        tx_to_l7_request(&tx)
                    });
                    if let (Some(sem), Some(li_cfg)) = (self.semantic, self.libinjection) {
                        if sem.enabled && sem.engines.semantic_first {
                            if let Some(hit) =
                                maybe_trace(&mut trace, "defender.core.semantic", |trace| {
                                    let _ = trace;
                                    inspect_semantic(
                                        sem,
                                        li_cfg,
                                        self.modules,
                                        req,
                                        self.semantic_stats.as_ref(),
                                    )
                                })
                            {
                                return self.finish(
                                    tx,
                                    with_stage(
                                        l7_hit_to_inspect_hit(InspectPhase::RequestBody, &hit),
                                        "core",
                                        "semantic",
                                    ),
                                );
                            }
                        }
                    }
                    if let Some(hit) =
                        maybe_trace(&mut trace, "defender.core.libinjection", |trace| {
                            let _ = trace;
                            self.run_plain_libinjection(req)
                        })
                    {
                        return self.finish(tx, with_stage(hit, "core", "libinjection"));
                    }
                }
                "crs" => {
                    if !api_sec_done {
                        api_sec_done = true;
                        if let Some(hit) =
                            maybe_trace(&mut trace, "defender.custom.api_sec", |trace| {
                                let _ = trace;
                                self.run_api_sec(&mut tx)
                            })
                        {
                            return self.finish(tx, hit);
                        }
                    }
                    let req = req.get_or_insert_with(|| {
                        tx.mark_phase(InspectPhase::RequestUpload);
                        tx.mark_phase(InspectPhase::RequestBody);
                        tx_to_l7_request(&tx)
                    });
                    if let Some(hit) = maybe_trace(&mut trace, "defender.expert", |trace| {
                        self.run_expert_stage(req, trace)
                    }) {
                        return self.finish(tx, hit);
                    }
                    if let Some(hit) =
                        maybe_trace(&mut trace, "defender.core.libinjection", |trace| {
                            let _ = trace;
                            self.run_plain_libinjection(req)
                        })
                    {
                        return self.finish(tx, with_stage(hit, "core", "libinjection"));
                    }
                }
                _ => {}
            }
        }

        let req = req.get_or_insert_with(|| {
            tx.mark_phase(InspectPhase::RequestUpload);
            tx.mark_phase(InspectPhase::RequestBody);
            tx_to_l7_request(&tx)
        });

        // Without semantic config (rare): still run plain libinjection to avoid going fully bare.
        if self.semantic.is_none() {
            if let Some(hit) =
                maybe_trace(&mut trace, "defender.core.libinjection", |trace| {
                    let _ = trace;
                    self.run_plain_libinjection(req)
                })
            {
                return self.finish(tx, with_stage(hit, "core", "libinjection"));
            }
        }

        if let Some(body) = self.response_body {
            tx.mark_phase(InspectPhase::ResponseBody);
            let level = self
                .semantic
                .map(|s| s.effective_expert_level(None))
                .unwrap_or(nerve_waf_semantic::ExpertLevel::Medium);
            if let Some(hit) =
                maybe_trace(&mut trace, "defender.expert.response", |trace| {
                    crate::expert_bridge::inspect_nerve_antibody_response(
                        req, body, level, trace,
                    )
                })
            {
                return self.finish(tx, with_stage(hit, "expert", "expert-native-response"));
            }
            if let Some(hit) =
                maybe_trace(&mut trace, "defender.custom.l7_response", |trace| {
                    let _ = trace;
                    self.hosted.engine().inspect_response(body)
                })
            {
                return self.finish(
                    tx,
                    with_stage(
                        l7_hit_to_inspect_hit(InspectPhase::ResponseBody, &hit),
                        "custom",
                        "l7-json",
                    ),
                );
            }
        }

        let mut verdict = InspectVerdict::pass();
        verdict.phases = tx.phases_run;
        verdict
    }

    fn run_api_sec(&self, tx: &mut WafTransaction) -> Option<InspectHit> {
        let api_sec = self.api_sec?;
        if !api_sec.enabled {
            return None;
        }
        tx.mark_phase(InspectPhase::RequestBody);
        let (path, query) = split_uri_path_query(&tx.uri);
        nerve_waf_api_sec::validate_request(
            api_sec,
            &tx.host,
            &tx.method,
            path,
            query,
            tx.body.len(),
            &tx.body,
        )
        .map(|v| {
            block_hit(
                InspectPhase::RequestBody,
                "api_sec",
                &format!("api-sec-{}", v.reason),
                "API Security Violation",
                "medium",
                "uri",
                &tx.uri,
                Some("api-sec"),
                Some("rust"),
            )
        })
    }

    fn run_pre_access(
        &self,
        tx: &mut WafTransaction,
        trace: &mut Option<&mut InspectTraceCtx>,
    ) -> Option<InspectHit> {
        let cc_enabled = self
            .modules
            .modules
            .get("cc")
            .map(|m| m.enabled)
            .unwrap_or(true);
        if let Some(cc) = self.cc {
            let cc_cfg = self.modules.merge_cc_config(&cc.config());
            if cc_enabled && cc_cfg.enabled {
                tx.mark_phase(InspectPhase::PreAccess);
                let result = maybe_trace(trace, "defender.pre_access.cc", |trace| {
                    let _ = trace;
                    cc.check_with_config(
                        &CcCheckRequest {
                            src_ip: tx.src_ip.clone(),
                            host: tx.host.clone(),
                            uri: tx.uri.clone(),
                        },
                        &cc_cfg,
                    )
                });
                if !result.allowed {
                    return Some(block_hit(
                        InspectPhase::PreAccess,
                        "cc",
                        "cc-limit",
                        "CC Rate Limit",
                        "medium",
                        "src_ip",
                        &tx.src_ip,
                        Some("cc"),
                        Some("rust"),
                    ));
                }
            }
        }

        if let Some(intel) = self.intel {
            tx.mark_phase(InspectPhase::PreAccess);
            let malicious = maybe_trace(trace, "defender.pre_access.intel", |trace| {
                let _ = trace;
                intel.is_malicious(&tx.src_ip)
            });
            if malicious {
                let cat = intel.category(&tx.src_ip).unwrap_or("intel");
                return Some(
                    block_hit(
                        InspectPhase::PreAccess,
                        "intel",
                        "intel-ip",
                        "Threat Intel Match",
                        "high",
                        "src_ip",
                        &tx.src_ip,
                        Some("intel"),
                        Some("rust"),
                    )
                    .with_category(cat),
                );
            }
        }

        if let Some(geo) = self.geo {
            tx.mark_phase(InspectPhase::PreAccess);
            let lookup = maybe_trace(trace, "defender.pre_access.geo", |trace| {
                let _ = trace;
                geo.lookup(&tx.src_ip)
            });
            if !lookup.allowed {
                return Some(block_hit(
                    InspectPhase::PreAccess,
                    "geo",
                    "geo-block",
                    "Geo Restriction",
                    "medium",
                    "src_ip",
                    &format!(
                        "{}:{}",
                        tx.src_ip,
                        lookup.country.unwrap_or_else(|| "XX".into())
                    ),
                    Some("geo"),
                    Some("rust"),
                ));
            }
        }

        let bot_module_on = self
            .modules
            .modules
            .get("bot")
            .map(|m| m.enabled)
            .unwrap_or(false);
        if self.bot.enabled && bot_module_on {
            tx.mark_phase(InspectPhase::PreAccess);
            let bot = maybe_trace(trace, "defender.pre_access.bot", |trace| {
                let _ = trace;
                nerve_waf_bot::score_with_config(&tx.user_agent, &tx.uri, &self.bot)
            });
            if bot.is_bot {
                let action = match self.bot.action.as_str() {
                    "block" => WafAction::Block,
                    "challenge" => WafAction::Challenge,
                    _ => WafAction::Log,
                };
                return Some(InspectHit {
                    phase: InspectPhase::PreAccess,
                    action,
                    category: "bot".into(),
                    rule_id: "bot-score".into(),
                    rule_name: "Bot Detection".into(),
                    severity: "low".into(),
                    matched_field: "user_agent".into(),
                    payload_sample: bot.reason,
                    plugin_name: Some("bot".into()),
                    plugin_type: Some("rust".into()),
                    param_path: None,
                    decode_chain: None,
                    semantic_score: None,
                    engines: None,
                    protection_stage: Some("pre_access".into()),
                    rule_set: Some("bot".into()),
                });
            }
        }

        if let Some(behavior) = self.behavior {
            if behavior.config().enabled {
                tx.mark_phase(InspectPhase::PreAccess);
                let bv = maybe_trace(trace, "defender.pre_access.behavior", |trace| {
                    let _ = trace;
                    behavior.score(&tx.src_ip, &tx.method, &tx.uri, &tx.user_agent)
                });
                if bv.is_anomaly {
                    let cfg = behavior.config();
                    let action = match cfg.action.as_str() {
                        "block" => WafAction::Block,
                        "challenge" => WafAction::Challenge,
                        _ => WafAction::Log,
                    };
                    return Some(InspectHit {
                        phase: InspectPhase::PreAccess,
                        action,
                        category: "behavior".into(),
                        rule_id: "behavior-anomaly".into(),
                        rule_name: "Behavior Anomaly".into(),
                        severity: "medium".into(),
                        matched_field: "src_ip".into(),
                        payload_sample: format!(
                            "score={} rpm={} paths={} reason={}",
                            bv.score, bv.request_count, bv.unique_paths, bv.reason
                        ),
                        plugin_name: Some("behavior".into()),
                        plugin_type: Some("rust".into()),
                        param_path: None,
                        decode_chain: None,
                        semantic_score: None,
                        engines: None,
                        protection_stage: Some("pre_access".into()),
                        rule_set: Some("behavior".into()),
                    });
                }
            }
        }

        None
    }

    /// Expert segment: Nerve-waf-expert first; if unloaded/miss, fall back to WASM/Coraza.
    fn run_expert_stage(
        &self,
        req: &InspectRequest,
        trace: &mut Option<&mut InspectTraceCtx>,
    ) -> Option<InspectHit> {
        let level = self
            .semantic
            .map(|s| s.effective_expert_level(None))
            .unwrap_or(nerve_waf_semantic::ExpertLevel::Medium);
        if let Some(hit) = crate::expert_bridge::inspect_nerve_antibody(req, level, trace) {
            return Some(with_stage(hit, "expert", "expert-native"));
        }
        if nerve_waf_wasm::wasm_crs_enabled() {
            match maybe_trace(trace, "defender.expert.wasm", |trace| {
                let _ = trace;
                nerve_waf_wasm::inspect_blocking(req)
            }) {
                Ok(Some(hit)) => {
                    return Some(with_stage(
                        l7_hit_to_inspect_hit(InspectPhase::RequestBody, &hit),
                        "expert",
                        "expert-crs",
                    ));
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "expert protection inspect failed");
                }
            }
        } else if nerve_waf_coraza::coraza_enabled() {
            match maybe_trace(trace, "defender.expert.coraza", |trace| {
                let _ = trace;
                nerve_waf_coraza::inspect_blocking(req)
            }) {
                Ok(Some(hit)) => {
                    return Some(with_stage(
                        l7_hit_to_inspect_hit(InspectPhase::RequestBody, &hit),
                        "expert",
                        "expert-crs",
                    ));
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "expert protection (compat) inspect failed");
                }
            }
        }
        None
    }

    fn run_custom_rules(&self, req: &InspectRequest) -> Option<InspectHit> {
        if self.custom_rules.is_empty() {
            return None;
        }
        let custom_enabled = self
            .modules
            .modules
            .get("custom")
            .map(|m| m.enabled)
            .unwrap_or(true);
        if !custom_enabled {
            return None;
        }
        nerve_waf_l7::inspect_custom_rules(self.custom_rules, req, &self.modules.mode).map(
            |hit| {
                let mut ih = l7_hit_to_inspect_hit(InspectPhase::RequestBody, &hit);
                if let Some(r) = self.custom_rules.iter().find(|r| r.id == hit.rule_id) {
                    ih.rule_set = Some(format!("custom:{}", r.rule_set_id));
                }
                ih
            },
        )
    }

    fn run_hosted_plugin(&self, req: &InspectRequest) -> Option<InspectHit> {
        if let Some(engine) = self.site_l7 {
            if let Some(hit) = engine.inspect_upload(req) {
                return Some(l7_hit_to_inspect_hit(
                    if hit.matched_field.contains("upload") {
                        InspectPhase::RequestUpload
                    } else {
                        InspectPhase::RequestBody
                    },
                    &hit,
                ));
            }
            if let Some(hit) = engine.inspect(req) {
                let phase = if hit.matched_field.contains("upload") {
                    InspectPhase::RequestUpload
                } else {
                    InspectPhase::RequestBody
                };
                return Some(l7_hit_to_inspect_hit(phase, &hit));
            }
        }
        let hit = self.hosted.inspect(req)?;
        let phase = if hit.matched_field.contains("upload") {
            InspectPhase::RequestUpload
        } else {
            InspectPhase::RequestBody
        };
        Some(l7_hit_to_inspect_hit(phase, &hit))
    }

    fn run_plain_libinjection(&self, req: &InspectRequest) -> Option<InspectHit> {
        let li_cfg = self.libinjection?;
        inspect_libinjection(li_cfg, self.modules, req)
            .map(|hit| l7_hit_to_inspect_hit(InspectPhase::RequestBody, &hit))
    }

    fn finish(&self, tx: WafTransaction, mut hit: InspectHit) -> InspectVerdict {
        if let Some(wl) = self.whitelist {
            let req = WhitelistRequest {
                host: &tx.host,
                uri: &tx.uri,
                src_ip: &tx.src_ip,
                rule_id: &hit.rule_id,
                category: &hit.category,
            };
            if is_whitelisted(wl, &req) {
                hit.action = WafAction::Log;
            }
        }
        // Global detect mode: log only, do not block. Core CRS / libinjection already softens in their resolve;
        // expert Antibody, pre_access (CC/Bot/Geo/Intel), api_sec etc. must soften here too,
        // else 'detect mode (log only)' would apply only to core while expert still block/challenge.
        hit.action = soften_action_for_detect_mode(&self.modules.mode, hit.action);
        let mut verdict = InspectVerdict::from_hit(hit);
        verdict.phases = tx.phases_run;
        verdict
    }
}

/// When `modules.mode == detect`, downgrade block/challenge to log (record only).
pub(crate) fn soften_action_for_detect_mode(mode: &str, action: WafAction) -> WafAction {
    if mode.eq_ignore_ascii_case("detect")
        && matches!(action, WafAction::Block | WafAction::Challenge)
    {
        WafAction::Log
    } else {
        action
    }
}

fn split_uri_path_query(uri: &str) -> (&str, &str) {
    match uri.split_once('?') {
        Some((p, q)) => (p, q),
        None => (uri, ""),
    }
}

fn block_hit(
    phase: InspectPhase,
    category: &str,
    rule_id: &str,
    rule_name: &str,
    severity: &str,
    field: &str,
    sample: &str,
    plugin: Option<&str>,
    ptype: Option<&str>,
) -> InspectHit {
    InspectHit {
        phase,
        action: WafAction::Block,
        category: category.into(),
        rule_id: rule_id.into(),
        rule_name: rule_name.into(),
        severity: severity.into(),
        matched_field: field.into(),
        payload_sample: sample.into(),
        plugin_name: plugin.map(str::to_string),
        plugin_type: ptype.map(str::to_string),
        param_path: None,
        decode_chain: None,
        semantic_score: None,
        engines: None,
        protection_stage: Some("pre_access".into()),
        rule_set: plugin.map(str::to_string).or_else(|| Some(category.into())),
    }
}

fn infer_rule_set(hit: &InspectHit, fallback: &str) -> String {
    if let Some(rs) = hit.rule_set.as_ref().filter(|s| !s.is_empty()) {
        return rs.clone();
    }
    match hit.plugin_name.as_deref() {
        Some("libinjection") => "libinjection".into(),
        Some("nerve-semantic") => hit
            .engines
            .as_ref()
            .and_then(|e| e.first())
            .cloned()
            .unwrap_or_else(|| "semantic".into()),
        Some("expert") | Some("coraza") => "expert-crs".into(),
        Some("custom-rules") => "custom".into(),
        Some(other) if other.starts_with("lua:") || hit.plugin_type.as_deref() == Some("lua") => {
            format!("lua:{other}")
        }
        Some(other) => other.into(),
        None => {
            // Embedded L7 JSON (xss-001 etc.) usually has no plugin_name.
            if hit.rule_id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                fallback.into()
            } else {
                "l7-json".into()
            }
        }
    }
}

fn with_stage(hit: InspectHit, stage: &str, rule_set_fallback: &str) -> InspectHit {
    let rs = infer_rule_set(&hit, rule_set_fallback);
    hit.with_protection_origin(stage, &rs)
}

trait HitPatch {
    fn with_category(self, category: &str) -> Self;
}

impl HitPatch for InspectHit {
    fn with_category(mut self, category: &str) -> Self {
        self.category = category.into();
        self
    }
}
