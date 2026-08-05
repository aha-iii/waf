//! L2：结构化研判结果 → LLM 自然语言解释（或模板回退）。

use nerve_waf_sdk::WafEventRecord;
use nerve_waf_store::{AiSettingsStore, TriageLlmConfig};
use nerve_waf_triage::{TriageResult, TriageVerdict};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Debug, Serialize)]
pub struct AiExplainResult {
    pub ai_summary: String,
    pub ai_engine: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_model: Option<String>,
}

struct CacheEntry {
    result: AiExplainResult,
    at: Instant,
}

pub struct TriageAiService {
    settings: Arc<AiSettingsStore>,
    http: reqwest::Client,
    cache: RwLock<HashMap<String, CacheEntry>>,
}

impl TriageAiService {
    pub fn new(settings: Arc<AiSettingsStore>) -> Arc<Self> {
        Arc::new(Self {
            settings,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            cache: RwLock::new(HashMap::new()),
        })
    }

    pub fn llm_config(&self) -> TriageLlmConfig {
        self.settings.triage_llm_config()
    }

    pub fn get_cached(&self, event_id: &str) -> Option<AiExplainResult> {
        let guard = self.cache.read().ok()?;
        let entry = guard.get(event_id)?;
        if entry.at.elapsed() > CACHE_TTL {
            return None;
        }
        Some(entry.result.clone())
    }

    pub fn merge_ai_into_json(triage_json: &mut Value, ai: &AiExplainResult) {
        if let Some(obj) = triage_json.as_object_mut() {
            obj.insert("ai_summary".into(), json!(ai.ai_summary));
            obj.insert("ai_engine".into(), json!(ai.ai_engine));
            if let Some(m) = &ai.ai_model {
                obj.insert("ai_model".into(), json!(m));
            }
        }
    }

    pub async fn explain(
        self: &Arc<Self>,
        event: &WafEventRecord,
        triage: &TriageResult,
        force_refresh: bool,
    ) -> AiExplainResult {
        if !force_refresh {
            if let Some(cached) = self.get_cached(&event.id) {
                return cached;
            }
        }

        let cfg = self.settings.triage_llm_config();
        let result = if cfg.llm_ready() {
            match self.call_llm(&cfg, event, triage).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("LLM explain failed event={} err={e}", event.id);
                    template_explain(event, triage)
                }
            }
        } else {
            template_explain(event, triage)
        };

        if let Ok(mut guard) = self.cache.write() {
            guard.insert(
                event.id.clone(),
                CacheEntry {
                    result: result.clone(),
                    at: Instant::now(),
                },
            );
        }
        result
    }

    pub fn spawn_background(
        self: &Arc<Self>,
        event: WafEventRecord,
        triage: TriageResult,
    ) {
        if !self.settings.triage_llm_config().auto_explain {
            return;
        }
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            let _ = svc.explain(&event, &triage, false).await;
        });
    }

    async fn call_llm(
        &self,
        cfg: &TriageLlmConfig,
        event: &WafEventRecord,
        triage: &TriageResult,
    ) -> Result<AiExplainResult, String> {
        let prompt = build_llm_prompt(event, triage);
        let body = json!({
            "model": cfg.model,
            "temperature": 0.2,
            "messages": [
                {
                    "role": "system",
                    "content": "你是企业 WAF/SOC 安全分析师。仅基于提供的结构化研判证据输出中文分析，不要臆造未给出的字段。输出 3-5 段：结论、关键证据、误报可能性、处置建议。"
                },
                { "role": "user", "content": prompt }
            ]
        });

        let resp = self
            .http
            .post(&cfg.api_url)
            .header("Authorization", format!("Bearer {}", cfg.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "HTTP {status}: {}",
                text.chars().take(300).collect::<String>()
            ));
        }

        let v: Value = resp.json().await.map_err(|e| e.to_string())?;
        let content = v
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "LLM response missing content".to_string())?;

        Ok(AiExplainResult {
            ai_summary: content.to_string(),
            ai_engine: "llm_v1".into(),
            ai_model: Some(cfg.model.clone()),
        })
    }
}

fn build_llm_prompt(event: &WafEventRecord, triage: &TriageResult) -> String {
    let evidence: Vec<String> = triage
        .evidence
        .iter()
        .map(|e| format!("- {} ({:+}): {}", e.signal, e.weight, e.detail))
        .collect();
    let recs = triage.recommendations.join("; ");
    format!(
        "WAF 事件研判（请勿编造额外 IOC）:\n\
         event_id={}\naction={} category={} severity={}\n\
         src_ip={} method={} uri={}\n\
         rule={} payload_sample={}\n\
         message={}\n\
         triage_engine={}\nverdict={:?} confidence={} risk_score={}\n\
         summary={}\n\
         evidence:\n{}\n\
         recommendations: {}",
        event.id,
        event.action,
        event.category,
        event.severity,
        event.src_ip.as_deref().unwrap_or("-"),
        event.method.as_deref().unwrap_or("-"),
        event.uri.as_deref().unwrap_or("-"),
        event
            .rule_name
            .as_deref()
            .or(event.rule_id.as_deref())
            .unwrap_or("-"),
        event.payload_sample.as_deref().unwrap_or("-"),
        event.message.as_deref().unwrap_or("-"),
        triage.engine,
        triage.verdict,
        triage.confidence,
        triage.risk_score,
        triage.summary,
        evidence.join("\n"),
        recs
    )
}

fn template_explain(event: &WafEventRecord, triage: &TriageResult) -> AiExplainResult {
    let verdict_cn = match triage.verdict {
        TriageVerdict::ConfirmedAttack => "确认攻击",
        TriageVerdict::LikelyAttack => "疑似攻击",
        TriageVerdict::NeedsReview => "待人工复核",
        TriageVerdict::LikelyFalsePositive => "疑似误报",
    };
    let top_ev: Vec<String> = triage
        .evidence
        .iter()
        .take(5)
        .map(|e| format!("「{}」({:+})：{}", e.signal, e.weight, e.detail))
        .collect();
    let rec = triage
        .recommendations
        .first()
        .cloned()
        .unwrap_or_else(|| "结合业务上下文复核该请求".into());

    let summary = format!(
        "【结论】{verdict_cn}（置信度 {}%，风险分 {}）。{}\n\n\
         【事件】{method} {uri} · 类型 {category} · 动作 {action} · 源 IP {ip}\n\n\
         【关键证据】{evidence}\n\n\
         【建议】{rec}\n\n\
         （模板解释；在 Console「AI 研判 → LLM 解释」启用 LLM 并保存 API Key）",
        triage.confidence,
        triage.risk_score,
        triage.summary,
        method = event.method.as_deref().unwrap_or("GET"),
        uri = event.uri.as_deref().unwrap_or("-"),
        category = event.category,
        action = event.action,
        ip = event.src_ip.as_deref().unwrap_or("-"),
        evidence = if top_ev.is_empty() {
            "无额外结构化证据".into()
        } else {
            top_ev.join("；")
        },
        rec = rec,
    );

    AiExplainResult {
        ai_summary: summary,
        ai_engine: "template_v1".into(),
        ai_model: None,
    }
}
