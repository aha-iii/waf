//! Edge dataplane generation (control-plane compile snapshot).

mod build;
pub mod bypass;
pub mod events;
pub mod generation;
mod hooks;
mod routes;
pub mod site_inspect;

pub use bypass::BypassInspectRuntime;
pub use build::compile_waf_edge_snapshot;
pub use events::post_waf_edge_events;
pub use generation::{bump_dataplane_generation, dataplane_generation, DataplaneGeneration};
pub use hooks::bump_after_mutation;
pub use routes::dataplane_router;
pub use site_inspect::{load_global_inspect_overlay, load_site_inspect_inputs, SiteInspectInputs};
