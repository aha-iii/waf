//! 数据面：拉控制面 snapshot → 检测 → 反代回源。

pub mod inspect;
pub mod proxy;
pub mod runtime;
pub mod sync;

pub use inspect::{new_shared_cache, SharedInspectCache};
pub use proxy::{health_handler, proxy_handler, EdgeState};
pub use runtime::{EdgeRuntime, SharedRuntime};
pub use sync::spawn_sync_loop;
