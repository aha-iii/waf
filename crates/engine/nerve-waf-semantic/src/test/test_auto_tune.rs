//! Tests extracted from `auto_tune/auto_tune.rs`.

use crate::auto_tune::*;
use crate::stats::SemanticFeedback;
use crate::{SemanticAutoTune, SemanticConfig};

    fn fp(param_path: &str, ts: u64) -> SemanticFeedback {
        SemanticFeedback {
            ts: ts.to_string(),
            site_id: "default".into(),
            event_id: None,
            verdict: "false_positive".into(),
            note: None,
            param_path: Some(param_path.into()),
            uri: None,
            applied_action: None,
        }
    }

    #[test]
    fn auto_whitelist_after_threshold() {
        let mut cfg = SemanticConfig::default();
        let tune = SemanticAutoTune {
            false_positive_threshold: 2,
            ..Default::default()
        };
        let history = vec![fp("query.token", 100)];
        let current = fp("query.token", 200);
        let r = apply_auto_tune(&mut cfg, &tune, &history, &current);
        assert!(r.config_changed);
        assert!(cfg.whitelist.param_names.contains(&"token".to_string()));
    }
