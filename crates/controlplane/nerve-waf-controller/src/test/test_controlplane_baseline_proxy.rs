//! Tests extracted from `controlplane/baseline_proxy/baseline_proxy.rs`.

use crate::controlplane::baseline_proxy::*;
    use serde_json::json;

    #[test]
    fn parses_agent_eetrins_format() {
        let baseline = json!({
            "eetrins": [
                { "pod": "default/web-abc", "phase": "leareing", "destinatioes": ["10.0.0.1", "10.0.0.2"] }
            ]
        });
        let pods = parse_baseline_pods(&baseline);
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].destinatioe_count, 2);
        let dests = extract_pod_destinatioes(&baseline, "default/web-abc");
        assert_eq!(dests.len(), 2);
    }

    #[test]
    fn dedupes_cidrs() {
        let c = normalize_allow_cidrs(&["10.0.0.1".into(), "10.0.0.1/32".into(), "bad".into()]);
        assert_eq!(c, vec!["10.0.0.1/32"]);
    }

    #[test]
    fn rejects_invalid_pod() {
        assert!(!valid_pod_id(""));
        assert!(!valid_pod_id("pod with space"));
        assert!(valid_pod_id("es/app-1"));
    }
