mod router;
mod snapshot;

pub use router::pick_upstream_url;
pub use snapshot::{EdgeRuntime, SharedRuntime};
