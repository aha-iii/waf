//! Tests extracted from `controlplane/ai_rate_limit/ai_rate_limit.rs`.

use crate::controlplane::ai_rate_limit::{check_expensive_ai, AiRateLimitKind};

    #[test]
    fn rate_limit_allows_under_cap() {
        for _ in 0..AiRateLimitKind::PolicySim.max_per_window() {
            assert!(check_expensive_ai(AiRateLimitKind::PolicySim).is_ok());
        }
        assert!(check_expensive_ai(AiRateLimitKind::PolicySim).is_err());
    }
