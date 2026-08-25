//! Tests extracted from `lib.rs`.

use crate::*;

    #[test]
    fn cidr_lookup_private() {
        let engine = GeoEngine::with_builtin(GeoConfig::default());
        let r = engine.lookup("10.1.2.3");
        assert_eq!(r.country.as_deref(), Some("PRIVATE"));
        assert_eq!(r.source.as_deref(), Some("cidr"));
    }
