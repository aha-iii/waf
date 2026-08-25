//! Tests extracted from `modules/modules.rs`.

use crate::modules::*;

    #[test]
    fn merge_and_diff_roundtrip() {
        let mut base = ProtectionModules::default();
        base.modules.get_mut("sqli").unwrap().enabled = true;
        base.modules.get_mut("webshell").unwrap().enabled = false;

        let mut effective = base.clone();
        effective.modules.get_mut("webshell").unwrap().enabled = true;
        effective
            .modules
            .get_mut("sqli")
            .unwrap()
            .disabled_rules
            .push("942100".into());

        let ov = ProtectionModules::diff_as_overrides(&base, &effective).expect("has overrides");
        assert!(ov.modules.contains_key("webshell"));
        assert!(ov.modules.contains_key("sqli"));
        let merged = ProtectionModules::merge_with_overrides(&base, &ov);
        assert_eq!(merged, effective);
    }

    #[test]
    fn diff_none_when_identical() {
        let base = ProtectionModules::default();
        assert!(ProtectionModules::diff_as_overrides(&base, &base).is_none());
    }
