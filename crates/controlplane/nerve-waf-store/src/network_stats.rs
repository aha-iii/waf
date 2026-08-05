use chrono::{DateTime, Utc};
use nerve_waf_sdk::WafEventRecord;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn aggregate_network_stats(events: &[WafEventRecord], days: i64) -> Value {
    let l3: Vec<_> = events.iter().filter(|e| e.layer == "l3").collect();
    let total = l3.len() as u64;
    let blocked = l3.iter().filter(|e| e.action == "block").count() as u64;

    let mut day_map: HashMap<String, u64> = HashMap::new();
    for e in &l3 {
        let day = e.ts.format("%Y-%m-%d").to_string();
        *day_map.entry(day).or_default() += 1;
    }
    let mut trend: Vec<_> = day_map
        .into_iter()
        .map(|(day, count)| json!({ "day": day, "count": count }))
        .collect();
    trend.sort_by(|a, b| {
        a["day"]
            .as_str()
            .unwrap_or("")
            .cmp(b["day"].as_str().unwrap_or(""))
    });

    let mut cat_map: HashMap<String, u64> = HashMap::new();
    for e in &l3 {
        *cat_map.entry(e.category.clone()).or_default() += 1;
    }
    let mut top_categories: Vec<_> = cat_map
        .into_iter()
        .map(|(category, count)| json!({ "category": category, "count": count }))
        .collect();
    top_categories.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    top_categories.truncate(10);

    let mut ip_map: HashMap<String, u64> = HashMap::new();
    for e in &l3 {
        let ip = e
            .dst_ip
            .as_deref()
            .or(e.src_ip.as_deref())
            .unwrap_or("");
        if !ip.is_empty() {
            *ip_map.entry(ip.to_string()).or_default() += 1;
        }
    }
    let mut top_ips: Vec<_> = ip_map
        .into_iter()
        .map(|(ip, count)| json!({ "ip": ip, "count": count }))
        .collect();
    top_ips.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    top_ips.truncate(10);

    let mut pod_map: HashMap<String, u64> = HashMap::new();
    for e in &l3 {
        if let Some(pod) = &e.pod {
            if !pod.is_empty() {
                *pod_map.entry(pod.clone()).or_default() += 1;
            }
        }
    }
    let mut top_pods: Vec<_> = pod_map
        .into_iter()
        .map(|(pod, count)| json!({ "pod": pod, "count": count }))
        .collect();
    top_pods.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    top_pods.truncate(10);

    json!({
        "total": total,
        "blocked": blocked,
        "trend": trend,
        "top_categories": top_categories,
        "top_ips": top_ips,
        "top_pods": top_pods,
        "days": days,
    })
}

pub fn file_network_stats(
    events: &[WafEventRecord],
    since: DateTime<Utc>,
    days: i64,
) -> Value {
    let l3: Vec<_> = events
        .iter()
        .filter(|e| e.layer == "l3" && e.ts >= since)
        .cloned()
        .collect();
    aggregate_network_stats(&l3, days)
}
