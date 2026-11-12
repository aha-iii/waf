#[cfg(feature = "inspect-trace")]
pub use nerve_inspect_trace::{
    finish_and_maybe_log, init_from_env, maybe_trace, trace_enabled, InspectTraceCtx,
    InspectTraceMeta,
};

#[cfg(not(feature = "inspect-trace"))]
mod noop {
    use nerve_waf_core::InspectTimingReport;

    pub struct InspectTraceCtx;

    impl InspectTraceCtx {
        pub fn new() -> Self {
            Self
        }
        pub fn is_active(&self) -> bool {
            false
        }
        pub fn sink(
            &self,
        ) -> Option<std::sync::Arc<std::sync::Mutex<Vec<nerve_waf_core::InspectNodeTiming>>>>
        {
            None
        }
        pub fn path_slot(&self) -> Option<std::sync::Arc<std::sync::Mutex<Option<String>>>> {
            None
        }
        pub fn into_report(self, _blocked: bool) -> Option<InspectTimingReport> {
            None
        }
        pub fn push(
            &mut self,
            _name: &'static str,
            _us: u64,
            _detail: Option<std::collections::HashMap<String, String>>,
        ) {
        }
    }

    pub struct InspectTraceMeta {
        pub method: String,
        pub host: String,
        pub uri: String,
        pub src_ip: String,
        pub trace_id: Option<String>,
    }

    pub fn init_from_env() {}
    pub fn trace_enabled() -> bool {
        false
    }
    pub fn maybe_trace<T>(
        ctx: &mut Option<&mut InspectTraceCtx>,
        _name: &'static str,
        f: impl FnOnce(&mut Option<&mut InspectTraceCtx>) -> T,
    ) -> T {
        f(ctx)
    }
    pub fn finish_and_maybe_log(
        report: InspectTimingReport,
        _meta: &InspectTraceMeta,
    ) -> InspectTimingReport {
        report
    }
    pub fn set_thread_sink(
        _: Option<std::sync::Arc<std::sync::Mutex<Vec<nerve_waf_core::InspectNodeTiming>>>>,
    ) {
    }
    pub fn set_thread_path(_: Option<std::sync::Arc<std::sync::Mutex<Option<String>>>>) {}
    pub fn clear_thread_sink() {}
}

#[cfg(not(feature = "inspect-trace"))]
pub use noop::*;
