//! Nerve WAF control-plane HTTP API (inspect, sites, events, modules, sync).
//!
//! Layout:
//! - `controlplane/` - Console HTTP, AppState, platform routes, middleware
//! - `dataplane/` - Edge snapshot compile plus List/Watch protocol

pub mod console_static;
pub mod controlplane;
pub mod dataplane;

// --- Stable public exports (shared lives under controlplane) ---
pub use controlplane::shared::AppState;
pub use controlplane::shared::{
    bootstrap, inspect_daemon, logging, phase11_middleware, platform, trace_bind, trace_middleware,
    triage_ai, triage_service,
};

// --- Legacy path aliases ---
pub use controlplane::ai_settings as ai_settings_routes;
pub use controlplane::app_crs as app_crs_routes;
pub use controlplane::custom_rules as custom_rules_routes;
pub use controlplane::events;
pub use controlplane::extensions as extensions_routes;
pub use controlplane::fp_remediation;
pub use controlplane::ingress as ingress_routes;
pub use controlplane::network as network_routes;
pub use controlplane::np as np_routes;
pub use controlplane::phase10 as phase10_routes;
pub use controlplane::phase12 as phase12_routes;
pub use controlplane::phase8 as phase8_routes;
pub use controlplane::plugin_l7 as plugin_l7_routes;
pub use controlplane::routes;
pub use controlplane::security as security_routes;
pub use controlplane::sites as platform_routes;
pub use controlplane::topology;
pub use controlplane::triage as triage_routes;
pub use dataplane::site_inspect;

// Legacy name.
pub use controlplane as control;
pub use controlplane::shared as shared;
