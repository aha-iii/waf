//! Tests extracted from `app_config/app_config.rs`.

use crate::app_config::*;
use nerve_waf_semantic::SemanticConfig;

    #[test]
    fn pipeline_file_roundtrip() {
        let dir = std::env::temp_dir().join("nerve_app_cfg_test");
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = SemanticConfig::default();
        save_app_pipeline_file(&dir, "demo", &cfg).unwrap();
        let loaded = load_app_pipeline_file(&dir, "demo").unwrap().unwrap();
        assert_eq!(loaded.enabled, cfg.enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bound_strategy_file_defaults() {
        let dir = std::env::temp_dir().join("nerve_bound_strat_test");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            load_bound_strategy_file(&dir, "demo").unwrap(),
            DEFAULT_BOUND_STRATEGY_ID
        );
        save_bound_strategy_file(&dir, "demo", "tpl-strict").unwrap();
        assert_eq!(load_bound_strategy_file(&dir, "demo").unwrap(), "tpl-strict");
        let _ = std::fs::remove_dir_all(&dir);
    }
