//! Tests extracted from `lib.rs`.

use crate::*;

#[test]
fn disabled_without_env() {
    std::env::remove_var("NERVE_WAF_CORAZA_URL");
    std::env::remove_var("NERVE_WAF_CRS_ENGINE");
    assert!(!coraza_enabled());
}
