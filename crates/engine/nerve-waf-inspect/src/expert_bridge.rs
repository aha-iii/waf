//! Expert engine bridge: `expert` feature links Nerve-antibody; otherwise no-op stubs.

#[cfg(feature = "expert")]
#[path = "expert_bridge_antibody.rs"]
mod expert_bridge_antibody;
#[cfg(feature = "expert")]
pub use expert_bridge_antibody::*;

#[cfg(not(feature = "expert"))]
mod stub {
    use nerve_waf_core::InspectHit;
    use nerve_waf_l7::InspectRequest;
    use nerve_waf_semantic::ExpertLevel as SemLevel;
    use std::path::Path;

    use crate::trace::InspectTraceCtx;

    pub fn expert_engine_enabled() -> bool {
        false
    }

    pub fn expert_last_load_error() -> Option<String> {
        Some("expert feature not enabled (rebuild with --features expert)".into())
    }

    pub fn reload_engine(_pack: Option<&Path>) -> Result<(), String> {
        Err("expert feature not enabled (rebuild with --features expert)".into())
    }

    pub fn expert_health() -> Option<()> {
        None
    }

    pub fn expert_metrics_snapshot() -> Option<()> {
        None
    }

    pub fn expert_license_status() -> Option<()> {
        None
    }

    pub fn inspect_nerve_antibody(
        _req: &InspectRequest,
        _level: SemLevel,
        _trace: &mut Option<&mut InspectTraceCtx>,
    ) -> Option<InspectHit> {
        None
    }

    pub fn inspect_nerve_antibody_response(
        _req: &InspectRequest,
        _response_body: &str,
        _level: SemLevel,
        _trace: &mut Option<&mut InspectTraceCtx>,
    ) -> Option<InspectHit> {
        None
    }
}

#[cfg(not(feature = "expert"))]
pub use stub::*;
