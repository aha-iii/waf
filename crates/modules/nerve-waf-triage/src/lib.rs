//! Explainable multi-signal triage for WAF events (SOC-style grading, not inline blocking).
//!
//! Combines behavior scores, rule confidence, and operator feedback; LLM is optional post-hoc only.

use nerve_waf_bot::{score_with_config, BotConfig};
use nerve_waf_sdk::WafEventRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriageVerdict {
    /// Multiple signals agree; treat as real attack.
    ConfirmedAttack,
    /// Likely attack; recommend block/alert.
    LikelyAttack,
    /// Conflicting signals or weak evidence; needs human review.
    NeedsReview,
    /// Likely false positive (static assets, business params, etc.).
    LikelyFalsePositive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageEvidence {
    pub signal: String,
    pub weight: i16,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageResult {
    pub verdict: TriageVerdict,
    /// 0–100 confidence in the verdict.
    pub confidence: u8,
    /// 0–100 attack risk score (related to but independent of verdict).
    pub risk_score: u8,
    pub summary: String,
    pub evidence: Vec<TriageEvidence>,
    pub recommendations: Vec<String>,
    /// Engine id for audit.
    pub engine: String,
}

#[derive(Debug, Clone, Default)]
pub struct TriageContext {
    /// Recent similar-event count for this IP (e.g. within 1h).
    pub ip_event_count: u32,
    pub ip_blocked_count: u32,
    pub intel_malicious: bool,
    pub intel_category: Option<String>,
    pub global_mode: String,
}

pub fn analyze_event(event: &WafEventRecord, ctx: &TriageContext) -> TriageResult {
    let mut score: i16 = 0;
    let mut evidence = Vec::new();
    let mut attack_signals = 0u8;
    let mut fp_signals = 0u8;

    if let Some(msg) = &event.message {
        if msg.contains("semantic_score=") || msg.contains("decode=") {
            score += 12;
            attack_signals += 1;
            evidence.push(evidence_row(
                "semantic_pipeline",
                12,
                "NSP 语义管线已命中（参数定位/解码/注入检测）",
            ));
        }
    }

    let action = event.action.to_ascii_lowercase();
    let severity = event.severity.to_ascii_lowercase();
    let category = event.category.to_ascii_lowercase();
    let uri = event.uri.as_deref().unwrap_or("");
    let payload = event.payload_sample.as_deref().unwrap_or("");
    let method = event.method.as_deref().unwrap_or("GET");
    let ua = event.user_agent.as_deref().unwrap_or("");

    // --- Rule/action signals ---
    if action == "block" {
        score += 35;
        attack_signals += 1;
        evidence.push(evidence_row("action_block", 35, "WAF 已执行拦截"));
    } else if action == "log" {
        score += 10;
        evidence.push(evidence_row(
            "action_log",
            10,
            "检测模式仅记录，需结合其他信号",
        ));
    }

    match severity.as_str() {
        "critical" => {
            score += 25;
            attack_signals += 1;
            evidence.push(evidence_row("severity_critical", 25, "规则严重度 critical"));
        }
        "high" => {
            score += 18;
            attack_signals += 1;
            evidence.push(evidence_row("severity_high", 18, "规则严重度 high"));
        }
        "medium" => score += 8,
        _ => score += 3,
    }

    match category.as_str() {
        "sqli" | "xss" | "rce" | "path_traversal" | "ssrf" | "webshell" | "prompt_injection" => {
            score += 22;
            attack_signals += 1;
            evidence.push(evidence_row(
                "category_owasp",
                22,
                &format!("Web 攻击：{category}"),
            ));
        }
        "cc" | "bot" | "intel" => {
            score += 15;
            attack_signals += 1;
            evidence.push(evidence_row(
                "category_abuse",
                15,
                &format!("滥用类：{category}"),
            ));
        }
        "l3_policy" | "anomaly" => {
            score += 12;
            evidence.push(evidence_row("category_network", 12, "网络/行为层异常"));
        }
        "custom" => {
            score += 10;
            evidence.push(evidence_row("category_custom", 10, "自定义签名命中"));
        }
        _ => {}
    }

    // --- Payload features ---
    if payload_has_attack_markers(payload, uri, &category) {
        score += 20;
        attack_signals += 1;
        evidence.push(evidence_row(
            "payload_markers",
            20,
            "URI/样本含典型攻击 token（union/select/script/.. 等）",
        ));
    }

    // --- Context ---
    if ctx.intel_malicious {
        score += 28;
        attack_signals += 1;
        let cat = ctx.intel_category.as_deref().unwrap_or("threat_intel");
        evidence.push(evidence_row(
            "intel_hit",
            28,
            &format!("威胁情报命中：{cat}"),
        ));
    }

    if ctx.ip_blocked_count >= 3 {
        score += 15;
        attack_signals += 1;
        evidence.push(evidence_row(
            "repeat_offender",
            15,
            &format!("源 IP 近期拦截 {} 次", ctx.ip_blocked_count),
        ));
    } else if ctx.ip_event_count >= 5 {
        score += 8;
        evidence.push(evidence_row(
            "ip_burst",
            8,
            &format!("源 IP 近期同类事件 {} 条", ctx.ip_event_count),
        ));
    }

    let bot = score_with_config(ua, uri, &BotConfig::default());
    if bot.is_bot && category != "bot" {
        score += 12;
        evidence.push(evidence_row(
            "bot_ua",
            12,
            &format!("自动化 UA 特征：{}", bot.reason),
        ));
    }

    // --- FP signals ---
    if is_static_asset(uri) {
        score -= 30;
        fp_signals += 1;
        evidence.push(evidence_row(
            "static_asset",
            -30,
            "请求路径为静态资源，常见 CRS 误报场景",
        ));
    }

    if ctx.global_mode == "detect" && action == "log" {
        score -= 8;
        fp_signals += 1;
        evidence.push(evidence_row("detect_only", -8, "全局检测模式，未实际拦截"));
    }

    if likely_benign_search(&category, uri, payload) {
        score -= 18;
        fp_signals += 1;
        evidence.push(evidence_row(
            "benign_search",
            -18,
            "参数形似正常搜索/分页，可能与业务冲突",
        ));
    }

    if method.eq_ignore_ascii_case("GET") && uri.len() < 80 && score < 40 && fp_signals == 0 {
        score -= 5;
        evidence.push(evidence_row("short_get", -5, "短 GET 请求，证据偏弱"));
    }

    let risk_score = score.clamp(0, 100) as u8;
    let verdict = verdict_from_score(score, attack_signals, fp_signals);
    let confidence = confidence_from_signals(attack_signals, fp_signals, score);
    let summary = summary_for(&verdict, &category, action.as_str());
    let recommendations = recommendations_for(&verdict, event, ctx);

    TriageResult {
        verdict,
        confidence,
        risk_score,
        summary,
        evidence,
        recommendations,
        engine: "multi_signal_fusion_v1".into(),
    }
}

fn evidence_row(signal: &str, weight: i16, detail: &str) -> TriageEvidence {
    TriageEvidence {
        signal: signal.into(),
        weight,
        detail: detail.into(),
    }
}

fn payload_has_attack_markers(payload: &str, uri: &str, category: &str) -> bool {
    let combined = format!("{} {}", uri, payload).to_ascii_lowercase();
    let markers = [
        "union select",
        "or 1=1",
        "1=1",
        "<script",
        "javascript:",
        "../",
        "..\\",
        "/etc/passwd",
        "cmd=",
        "exec(",
        "ignore previous instructions",
        "system prompt",
    ];
    if markers.iter().any(|m| combined.contains(m)) {
        return true;
    }
    matches!(category, "sqli" | "xss" | "rce" | "prompt_injection")
        && !combined.trim().is_empty()
        && combined.len() > 8
}

fn is_static_asset(uri: &str) -> bool {
    let path = uri.split('?').next().unwrap_or(uri).to_ascii_lowercase();
    [
        ".js", ".css", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".woff", ".woff2", ".ico", ".map",
        ".webp",
    ]
    .iter()
    .any(|ext| path.ends_with(ext))
}

fn likely_benign_search(category: &str, uri: &str, payload: &str) -> bool {
    if !matches!(category, "sqli" | "xss") {
        return false;
    }
    let q = uri.to_ascii_lowercase();
    let benign_keys = [
        "q=", "query=", "search=", "keyword=", "page=", "sort=", "name=",
    ];
    if !benign_keys.iter().any(|k| q.contains(k)) {
        return false;
    }
    let sample = payload.to_ascii_lowercase();
    !sample.contains("union") && !sample.contains("<script") && !sample.contains("../")
}

fn verdict_from_score(score: i16, attack: u8, fp: u8) -> TriageVerdict {
    if score >= 65 && attack >= 2 {
        TriageVerdict::ConfirmedAttack
    } else if score >= 40 || (score >= 30 && attack >= 1) {
        TriageVerdict::LikelyAttack
    } else if fp >= 1 && score < 35 {
        TriageVerdict::LikelyFalsePositive
    } else {
        TriageVerdict::NeedsReview
    }
}

fn confidence_from_signals(attack: u8, fp: u8, score: i16) -> u8 {
    let base = 50i16;
    let boost = attack.saturating_mul(12) as i16 + fp.saturating_mul(8) as i16;
    let align = if (score >= 50 && attack >= 2) || (score <= 20 && fp >= 1) {
        15i16
    } else {
        0
    };
    (base + boost + align).clamp(40, 95) as u8
}

fn summary_for(verdict: &TriageVerdict, category: &str, action: &str) -> String {
    match verdict {
        TriageVerdict::ConfirmedAttack => {
            format!("多信号一致，判定为真实 {category} 攻击（动作 {action}）")
        }
        TriageVerdict::LikelyAttack => {
            format!("较大概率为 {category} 攻击，建议保留拦截并关注同源 IP")
        }
        TriageVerdict::NeedsReview => {
            "证据不充分或信号冲突，建议人工复核后再调整规则/白名单".into()
        }
        TriageVerdict::LikelyFalsePositive => {
            "较大概率为误报（如静态资源/业务参数），可考虑按 URL 或规则 ID 加白".to_string()
        }
    }
}

fn recommendations_for(
    verdict: &TriageVerdict,
    event: &WafEventRecord,
    ctx: &TriageContext,
) -> Vec<String> {
    let mut rec = Vec::new();
    match verdict {
        TriageVerdict::ConfirmedAttack | TriageVerdict::LikelyAttack => {
            rec.push("保留当前拦截策略".into());
            if ctx.ip_blocked_count >= 2 {
                rec.push("考虑将源 IP 加入应急封禁列表".into());
            }
            if let Some(rid) = &event.rule_id {
                rec.push(format!("关注规则 {rid} 在同路径的命中率"));
            }
        }
        TriageVerdict::NeedsReview => {
            rec.push("在检测模式下观察 24h，确认业务是否受影响".into());
            rec.push("使用规则测试/回放验证该 URI 样本".into());
            if let Some(rid) = &event.rule_id {
                rec.push(format!("评估是否为规则 {rid} 误报，必要时添加例外"));
            }
        }
        TriageVerdict::LikelyFalsePositive => {
            rec.push("建议添加 URL/参数白名单或降低该路径灵敏度".into());
            if event.action == "block" {
                rec.push("可临时切换检测模式验证业务流量".into());
            }
        }
    }
    rec
}

#[cfg(test)]
mod test;
