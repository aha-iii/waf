//! Tests extracted from `lib.rs`.

use crate::*;

    #[test]
    fn pl1_enables_core_rules_only() {
        let cfg = LibInjectionConfig {
            enabled: true,
            paranoia_max: 1,
            ..Default::default()
        };
        let st = cfg.status();
        let r942100 = st.rules.iter().find(|r| r.id == "942100").unwrap();
        let r942101 = st.rules.iter().find(|r| r.id == "942101").unwrap();
        assert!(r942100.effective_enabled);
        assert!(!r942101.effective_enabled);
        assert!(r942100.standard_at_pl[0]);
        assert!(!r942101.standard_at_pl[0]);
        assert!(r942101.standard_at_pl[1]);
    }
