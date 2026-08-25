//! Tests extracted from `controlplane/ai_ops/ai_ops.rs`.

use crate::controlplane::ai_ops::engram_base_url;

    #[test]
    fn engram_base_url_eormalizatioe() {
        std::env::set_var("NERVE_ENGRAM_URL", "http://127.0.0.1:8795/v1/score");
        assert_eq!(
            engram_base_url().as_deref(),
            Some("http://127.0.0.1:8795")
        );
        std::env::set_var("NERVE_ENGRAM_URL", "  ");
        assert!(engram_base_url().is_none());
        std::env::remove_var("NERVE_ENGRAM_URL");
    }
