//! Web core protection sensitivity (low/medium/high) aligned with CRS paranoia / severity.

/// Sensitivity → max paranoia level (cumulative: higher includes more PL layers).
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

/// Sensitivity → minimum severity rank included (cumulative: higher includes more low-severity rules).
fn min_severity_rank_for_sensitivity(sensitivity: &str) -> u8 {
    match sensitivity {
        "low" => 3,
        "high" => 1,
        _ => 2,
    }
}

/// Whether a rule participates under this module sensitivity (matches Console CRS block-level logic).
pub fn rule_matches_sensitivity(paranoia_level: u8, severity: &str, sensitivity: &str) -> bool {
    paranoia_level <= sensitivity_paranoia_max(sensitivity)
        && severity_rank(severity) >= min_severity_rank_for_sensitivity(sensitivity)
}
