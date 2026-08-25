//! Unit tests physically separated from production modules.

mod test_pipeline;
#[cfg(feature = "expert")]
mod test_expert_bridge;
