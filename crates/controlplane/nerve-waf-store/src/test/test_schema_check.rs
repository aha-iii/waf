//! Tests extracted from `schema_check/schema_check.rs`.

use crate::schema_check::ai_required_tables;

#[test]
fn ai_required_tables_cover_posture_and_labels() {
    let tables = ai_required_tables();
    assert!(tables.contains(&"waf_ml_labels"));
    assert!(tables.contains(&"waf_posture_scan"));
    assert!(tables.contains(&"waf_fp_remediation"));
}
