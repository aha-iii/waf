//! Contract and parity tests (separated from production `snapshot.rs` / `build.rs`).
//!
//! 注意：涉及 `nerve_waf_embed` 的 parity 测试在 `tests/parity.rs`（integration test），
//! 因 embed 反向依赖本 crate，单元测试会导致类型重复编译。

mod test_snapshot_contract;
