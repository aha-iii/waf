//! SIEM / CEF 导出

use chrono::SecondsFormat;
use nerve_waf_sdk::WafEventRecord;

pub fn to_cef(event: &WafEventRecord) -> String {
    let severity = cef_severity(&event.severity);
    let rule = event.rule_name.as_deref().unwrap_or("rule");
    let src = event.src_ip.as_deref().unwrap_or("-");
    let dst = event.dst_ip.as_deref().unwrap_or("-");
    let request = event.uri.as_deref().unwrap_or("-");
    let msg = event.message.as_deref().unwrap_or("-");
    format!(
        "CEF:0|Nerve|WAF|1.0|{}|{}|{}|src={} dst={} request={} act={} cat={} site={} msg={}",
        event.rule_id.as_deref().unwrap_or("waf"),
        rule,
        severity,
        src,
        dst,
        request,
        event.action,
        event.category,
        event.site_id,
        msg.replace('|', "/")
    )
}

pub fn events_to_cef_lines(events: &[WafEventRecord]) -> String {
    events.iter().map(to_cef).collect::<Vec<_>>().join("\n")
}

pub fn events_json(events: &[WafEventRecord]) -> serde_json::Value {
    serde_json::json!({
        "exported_at": chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "count": events.len(),
        "events": events,
    })
}

fn cef_severity(s: &str) -> u8 {
    match s.to_lowercase().as_str() {
        "critical" => 10,
        "high" => 8,
        "medium" => 5,
        "low" => 3,
        _ => 1,
    }
}
