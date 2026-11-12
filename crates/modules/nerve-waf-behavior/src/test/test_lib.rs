//! Tests extracted from `lib.rs`.

use crate::*;

#[test]
fn detects_path_scan() {
    let cfg = BehaviorConfig {
        enabled: true,
        score_threshold: 50,
        max_unique_paths: 3,
        ..Default::default()
    };
    let mut map = HashMap::new();
    for i in 0..5 {
        let v = score_with_state(
            &cfg,
            &mut map,
            "10.0.0.1",
            "GET",
            &format!("/path{i}"),
            "Mozilla/5.0",
        );
        if i >= 3 {
            assert!(v.score > 0, "expected score at i={i}");
        }
    }
}
