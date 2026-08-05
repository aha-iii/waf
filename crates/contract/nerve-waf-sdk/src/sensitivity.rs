//! Web 核心防护「灵敏度 低/中/高」与 CRS 规则 paranoia / 严重度对齐。

/// 灵敏度 → 最大 paranoia level（累积：越高包含越多 PL 层）
pub fn sensitivity_paranoia_max(sensitivity: &str) -> u8 {
    match sensitivity {
        "low" => 1,
        "high" => 4,
        _ => 2, // medium
    }
}

pub fn severity_rank(severity: &str) -> u8 {
    match severity.to_ascii_lowercase().as_str() {
        "critical" | "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

/// 灵敏度 → 最低纳入的严重度 rank（累积：越高纳入越多低危规则）
fn min_severity_rank_for_sensitivity(sensitivity: &str) -> u8 {
    match sensitivity {
        "low" => 3,
        "high" => 1,
        _ => 2,
    }
}

/// 规则是否在该模块灵敏度下参与检测（与 Console CRS 页拦截等级逻辑一致）
pub fn rule_matches_sensitivity(paranoia_level: u8, severity: &str, sensitivity: &str) -> bool {
    paranoia_level <= sensitivity_paranoia_max(sensitivity)
        && severity_rank(severity) >= min_severity_rank_for_sensitivity(sensitivity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_sensitivity_high_severity_pl1_only() {
        assert!(rule_matches_sensitivity(1, "critical", "low"));
        assert!(!rule_matches_sensitivity(2, "critical", "low"));
        assert!(!rule_matches_sensitivity(1, "medium", "low"));
    }

    #[test]
    fn high_sensitivity_includes_all() {
        assert!(rule_matches_sensitivity(4, "low", "high"));
        assert!(rule_matches_sensitivity(2, "medium", "high"));
    }

    #[test]
    fn medium_cumulative() {
        assert!(rule_matches_sensitivity(2, "medium", "medium"));
        assert!(!rule_matches_sensitivity(3, "medium", "medium"));
        assert!(!rule_matches_sensitivity(2, "low", "medium"));
    }
}
