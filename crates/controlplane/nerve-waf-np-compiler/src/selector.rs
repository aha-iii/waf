//! K8s label / namespaceSelector matching (P0).

use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NamespaceSnapshot {
    pub name: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// matchLabels on a pod or namespace object.
pub fn labels_match(selector: &Value, labels: &HashMap<String, String>) -> bool {
    let Some(obj) = selector.get("matchLabels").and_then(|v| v.as_object()) else {
        return selector.as_object().map(|o| o.is_empty()).unwrap_or(true);
    };
    if obj.is_empty() {
        return true;
    }
    obj.iter().all(|(k, v)| {
        labels
            .get(k)
            .map(|lv| v.as_str().map(|sv| lv == sv).unwrap_or(false))
            .unwrap_or(false)
    })
}

/// matchExpressions (In / NotIn / Exists / DoesNotExist) — subset.
pub fn labels_match_full(selector: &Value, labels: &HashMap<String, String>) -> bool {
    if labels_match(selector, labels) {
        return true;
    }
    let Some(exprs) = selector.get("matchExpressions").and_then(|v| v.as_array()) else {
        return false;
    };
    for expr in exprs {
        let key = expr.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let op = expr
            .get("operator")
            .and_then(|v| v.as_str())
            .unwrap_or("In");
        let vals: Vec<&str> = expr
            .get("values")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        let label_val = labels.get(key).map(|s| s.as_str());
        match op {
            "In" => {
                if label_val.map(|v| vals.contains(&v)).unwrap_or(false) {
                    continue;
                }
                return false;
            }
            "NotIn" => {
                if label_val.map(|v| !vals.contains(&v)).unwrap_or(true) {
                    continue;
                }
                return false;
            }
            "Exists" => {
                if label_val.is_some() {
                    continue;
                }
                return false;
            }
            "DoesNotExist" => {
                if label_val.is_none() {
                    continue;
                }
                return false;
            }
            _ => return false,
        }
    }
    true
}

/// namespaceSelector on an NP peer: matches namespace **labels** of the pod's namespace.
pub fn namespace_selector_matches(
    ns_selector: &Value,
    pod_namespace: &str,
    all_namespaces: &[NamespaceSnapshot],
) -> bool {
    if ns_selector.is_null()
        || ns_selector.as_object().map(|o| o.is_empty()).unwrap_or(true)
    {
        return true;
    }
    let ns = all_namespaces
        .iter()
        .find(|n| n.name == pod_namespace);
    match ns {
        Some(n) => labels_match_full(ns_selector, &n.labels),
        None => false,
    }
}
