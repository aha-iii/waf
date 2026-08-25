//! Dataplane: pull control-plane snapshot → inspect → reverse-proxy to origin.

pub mod inspect;
pub mod proxy;
pub mod runtime;
pub mod sync;
pub(crate) mod traffic;

pub use inspect::{new_shared_cache, SharedInspectCache};
pub use proxy::{health_handler, proxy_handler, EdgeState};
pub use runtime::{EdgeRuntime, SharedRuntime};
pub use sync::spawn_sync_loop;
