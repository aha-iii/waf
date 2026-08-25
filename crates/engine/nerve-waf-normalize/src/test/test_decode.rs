//! Tests extracted from `decode/decode.rs`.

use crate::decode::*;

    #[test]
    fn double_url_decode() {
        let layers = decode_chain("/%252fetc%252fpasswd", &DecodePolicy::default());
        assert!(layers.len() >= 2);
        assert!(
            layers.last().unwrap().text.contains("/etc/passwd")
                || layers.last().unwrap().text.contains("%2f")
        );
    }

    #[test]
    fn html_entity_decode() {
        let layers = decode_chain(
            "&lt;script&gt;alert(1)&lt;/script&gt;",
            &DecodePolicy::default(),
        );
        assert!(layers.iter().any(|l| l.text.contains("<script>")));
    }

    #[test]
    fn hex_slash_x() {
        let mut cfg = DecodePolicy::default();
        cfg.hex = true;
        cfg.order = vec!["hex".into()];
        let layers = decode_chain(r"\x27 OR 1=1", &cfg);
        assert!(layers.iter().any(|l| l.text.contains("'")));
    }

    #[test]
    fn chr_decode() {
        let mut cfg = DecodePolicy::default();
        cfg.chr = true;
        cfg.order = vec!["chr".into()];
        let layers = decode_chain("char(39) OR 1=1", &cfg);
        assert!(layers.iter().any(|l| l.text.contains("'")));
    }
