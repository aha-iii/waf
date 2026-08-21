//! L7 regex/OWASP rule engine (embedded rules plus JSON packs).

mod custom;
mod engine;
mod rules;
mod scan_targets;

pub use custom::{inspect_custom_rules, CustomRule, CustomRuleSet, CustomRulesFile};
pub use engine::{InspectRequest, InspectResult, L7Engine};
pub use rules::{
    compile_rules, compile_rules_from_json, embedded_rules, CompiledRule, CompiledStep, RawChainStep,
    RawRule, RuleCompileError,
};
pub use scan_targets::{is_chain_only_rule, parse_scan_targets, ScanTarget, CHAIN_ONLY_RULE_IDS};

pub fn default_engine() -> L7Engine {
    L7Engine::from_rules(ProtectionModules::default(), embedded_rules())
}

pub fn rule_count() -> usize {
    embedded_rules().len()
}

use nerve_waf_sdk::ProtectionModules;

#[cfg(test)]
mod test;
