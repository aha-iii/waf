//! Tests extracted from `identity/identity.rs`.

use crate::identity::*;

#[test]
fn stable_id() {
    assert_eq!(
        security_id_for("default", "web"),
        security_id_for("default", "web")
    );
}
