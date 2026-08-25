//! Alert dispatch helpers (legacy/local multi-channel fan-out).
//!
//! Prefer the Nerve-alert product for production messaging.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeComConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    #[serde(default = "default_true")]
    pub use_tls: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: Vec<String>,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            use_tls: true,
            username: String::new(),
            password: String::new(),
            from: String::new(),
            to: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SmsConfig {
    #[serde(default)]
    pub enabled: bool,
    /// webhook: HTTP gateway; aliyun: Aliyun SMS (needs access_key/secret/template).
    #[serde(default = "default_sms_provider")]
    pub provider: String,
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
    #[serde(default)]
    pub sign_name: String,
    #[serde(default)]
    pub template_code: String,
    #[serde(default)]
    pub phones: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertTemplateConfig {
    /// Email subject / alert title; supports `${varname}` placeholders.
    #[serde(default = "default_tpl_title")]
    pub title: String,
    /// Email body, SMS, or Webhook plain text.
    #[serde(default = "default_tpl_body")]
    pub body: String,
    /// WeCom Markdown message body.
    #[serde(default = "default_tpl_wecom")]
    pub wecom_markdown: String,
}

impl Default for AlertTemplateConfig {
    fn default() -> Self {
        Self {
            title: default_tpl_title(),
            body: default_tpl_body(),
            wecom_markdown: default_tpl_wecom(),
        }
    }
}

fn default_tpl_title() -> String {
    "[WAF告警] ${severity_label} · ${category_label} · ${action_label}".into()
}

fn default_tpl_body() -> String {
    "━━━━━━━━━━━━━━━━━━━━━━\n\
     Nerve WAF · 安全告警\n\
     ━━━━━━━━━━━━━━━━━━━━━━\n\
     \n\
     【事件概要】\n\
       严重级别：${severity_label} (${severity})\n\
       攻击类型：${category_label}\n\
       处置动作：${action_label}\n\
     \n\
     【请求信息】\n\
       源 IP   ：${src_ip}\n\
       防护应用：${site_id}\n\
       域名    ：${host}\n\
       请求路径：${uri}\n\
       详情    ：${message}\n\
     \n\
     【时间】${time}\n\
     \n\
     【链路追踪】\n\
       Trace ID：${trace_id}\n\
       Span ID ：${span_id}"
        .into()
}

fn default_tpl_wecom() -> String {
    "## 🛡️ WAF 安全告警\n\
     \n\
     > **严重级别** <font color=\"warning\">${severity_label}</font>　**处置** <font color=\"comment\">${action_label}</font>\n\
     > **攻击类型** ${category_label}\n\
     \n\
     ---\n\
     \n\
     > **源 IP** ${src_ip}　**应用** ${site_id}\n\
     > **域名** ${host}　**时间** ${time}\n\
     \n\
     > **请求路径**\n\
     > `${uri}`\n\
     \n\
     > **规则说明** ${message}\n\
     \n\
     ---\n\
     \n\
     > **链路追踪**\n\
     > Trace `${trace_id}`\n\
     > Span `${span_id}`"
        .into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Generic webhook (legacy-config compatible).
    #[serde(default)]
    pub webhooks: Vec<String>,
    #[serde(default = "default_min_severity")]
    pub min_severity: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub wecom: WeComConfig,
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub sms: SmsConfig,
    #[serde(default)]
    pub template: AlertTemplateConfig,
    /// Do not re-push same attack-type+src-IP within cooldown (seconds; 0 = unlimited).
    #[serde(default = "default_cooldown_sec")]
    pub cooldown_sec: u32,
    #[serde(default)]
    pub digest: AlertDigestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertDigestConfig {
    /// Daily security digest.
    #[serde(default = "default_true")]
    pub daily_enabled: bool,
    /// UTC hour 0–23 (default 0 = 08:00 Beijing time).
    #[serde(default)]
    pub daily_hour_utc: u8,
    /// Weekly security digest.
    #[serde(default)]
    pub weekly_enabled: bool,
    /// 0 = Monday … 6 = Sunday.
    #[serde(default)]
    pub weekly_weekday: u8,
    #[serde(default = "default_weekly_hour")]
    pub weekly_hour_utc: u8,
    /// Re-check for un-ACKed alerts.
    #[serde(default = "default_true")]
    pub unack_enabled: bool,
    /// Remind if still unacked after N minutes.
    #[serde(default = "default_unack_after")]
    pub unack_after_minutes: u32,
    /// Re-check scan interval (seconds).
    #[serde(default = "default_unack_interval")]
    pub unack_interval_sec: u32,
    /// Max reminders per alert.
    #[serde(default = "default_unack_max")]
    pub unack_remind_max: u32,
}

impl Default for AlertDigestConfig {
    fn default() -> Self {
        Self {
            daily_enabled: true,
            daily_hour_utc: 0,
            weekly_enabled: true,
            weekly_weekday: 0,
            weekly_hour_utc: 0,
            unack_enabled: true,
            unack_after_minutes: default_unack_after(),
            unack_interval_sec: default_unack_interval(),
            unack_remind_max: default_unack_max(),
        }
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhooks: Vec::new(),
            min_severity: default_min_severity(),
            categories: Vec::new(),
            wecom: WeComConfig::default(),
            email: EmailConfig::default(),
            sms: SmsConfig::default(),
            template: AlertTemplateConfig::default(),
            cooldown_sec: default_cooldown_sec(),
            digest: AlertDigestConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPayload {
    pub ts: DateTime<Utc>,
    pub severity: String,
    pub category: String,
    pub action: String,
    pub src_ip: Option<String>,
    pub host: Option<String>,
    pub uri: Option<String>,
    pub message: String,
    pub site_id: Option<String>,
    /// Nerve-Trace-Id for cross-component log correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Nerve-Span-Id for this inspect span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertLogRecord {
    pub id: String,
    pub ts: DateTime<Utc>,
    /// realtime / test / ingest
    pub trigger: String,
    /// sent / suppressed / skipped / failed / no_channel
    pub status: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    pub severity: String,
    pub category: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    pub delivered: usize,
    #[serde(default)]
    pub channels: Vec<ChannelResult>,
    #[serde(default)]
    pub acked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acked_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acked_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ack_reason: Option<String>,
    #[serde(default)]
    pub remind_count: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DispatchResult {
    pub delivered: usize,
    pub channels: Vec<ChannelResult>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelResult {
    pub channel: String,
    pub ok: bool,
    pub detail: String,
}

pub struct AlertDispatcher {
    config: Arc<std::sync::RwLock<AlertConfig>>,
    http: reqwest::Client,
    cooldown: Arc<std::sync::RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl AlertDispatcher {
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config: Arc::new(std::sync::RwLock::new(normalize_config(config))),
            http: reqwest::Client::new(),
            cooldown: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    pub fn update_config(&self, config: AlertConfig) {
        if let Ok(mut c) = self.config.write() {
            *c = normalize_config(config);
        }
    }

    pub fn config(&self) -> AlertConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn has_channels(&self) -> bool {
        has_any_channel(&self.config())
    }

    pub fn should_alert(&self, payload: &AlertPayload) -> bool {
        self.skip_reason(payload).is_none()
    }

    pub fn skip_reason(&self, payload: &AlertPayload) -> Option<String> {
        let cfg = self.config();
        if !cfg.enabled {
            return Some("告警未启用".into());
        }
        if !has_any_channel(&cfg) {
            return Some("未配置通知通道".into());
        }
        let is_enforcement =
            payload.action == "block" || payload.action == "challenge";
        if !is_enforcement
            && severity_rank(&payload.severity) < severity_rank(&cfg.min_severity)
        {
            return Some(format!(
                "严重级别 {} 低于阈值 {}",
                payload.severity, cfg.min_severity
            ));
        }
        None
    }

    pub async fn dispatch(&self, payload: &AlertPayload) -> usize {
        self.dispatch_detailed(payload).await.delivered
    }

    pub async fn dispatch_detailed(&self, payload: &AlertPayload) -> DispatchResult {
        if let Some(reason) = self.skip_reason(payload) {
            return DispatchResult {
                status: "skipped".into(),
                detail: reason,
                ..Default::default()
            };
        }
        if let Some(result) = self.check_cooldown(payload) {
            return result;
        }
        let mut result = self.dispatch_all(payload).await;
        if result.delivered > 0 {
            self.mark_cooldown(payload);
            result.status = "sent".into();
            result.detail = format!("已推送 {} 个通道", result.delivered);
        } else {
            result.status = "failed".into();
            result.detail = "全部通道发送失败".into();
        }
        result
    }

    /// Digest / recheck push: skip policy & cooldown; requires enabled + channel.
    pub async fn dispatch_digest(&self, payload: &AlertPayload) -> DispatchResult {
        let cfg = self.config();
        if !cfg.enabled {
            return DispatchResult {
                status: "skipped".into(),
                detail: "告警未启用".into(),
                ..Default::default()
            };
        }
        if !has_any_channel(&cfg) {
            return DispatchResult {
                status: "no_channel".into(),
                detail: "未配置通知通道".into(),
                ..Default::default()
            };
        }
        let mut result = self.dispatch_all(payload).await;
        result.status = if result.delivered > 0 {
            "sent".into()
        } else {
            "failed".into()
        };
        result.detail = if result.delivered > 0 {
            format!("摘要已推送 {} 个通道", result.delivered)
        } else {
            "摘要推送失败".into()
        };
        result
    }

    /// Test alert: skip enabled / type / severity / cooldown; only verify channel reachability.
    pub async fn dispatch_test(&self, payload: &AlertPayload) -> DispatchResult {
        let cfg = self.config();
        if !has_any_channel(&cfg) {
            return DispatchResult {
                status: "no_channel".into(),
                detail: "未配置通知通道".into(),
                ..Default::default()
            };
        }
        let mut result = self.dispatch_all(payload).await;
        result.status = if result.delivered > 0 {
            "sent".into()
        } else {
            "failed".into()
        };
        result.detail = if result.delivered > 0 {
            format!("测试消息已推送 {} 个通道", result.delivered)
        } else {
            "测试消息发送失败".into()
        };
        result
    }

    fn check_cooldown(&self, payload: &AlertPayload) -> Option<DispatchResult> {
        let cfg = self.config();
        if cfg.cooldown_sec == 0 {
            return None;
        }
        let key = cooldown_key(payload);
        let map = self.cooldown.read().ok()?;
        let last = map.get(&key)?;
        let elapsed = Utc::now().signed_duration_since(*last).num_seconds();
        if elapsed < cfg.cooldown_sec as i64 {
            return Some(DispatchResult {
                status: "suppressed".into(),
                detail: format!(
                    "冷却中（{} 秒内同类型+同源 IP 不重复推送，剩余约 {} 秒）",
                    cfg.cooldown_sec,
                    cfg.cooldown_sec as i64 - elapsed
                ),
                ..Default::default()
            });
        }
        None
    }

    fn mark_cooldown(&self, payload: &AlertPayload) {
        let cfg = self.config();
        if cfg.cooldown_sec == 0 {
            return;
        }
        if let Ok(mut map) = self.cooldown.write() {
            map.insert(cooldown_key(payload), Utc::now());
        }
    }

    async fn dispatch_all(&self, payload: &AlertPayload) -> DispatchResult {
        let cfg = self.config();
        let text = render_template(&cfg.template.body, payload);
        let title = render_template(&cfg.template.title, payload);
        let wecom_md = render_template(&cfg.template.wecom_markdown, payload);
        let mut channels = Vec::new();

        if cfg.wecom.enabled && !cfg.wecom.webhook_url.is_empty() {
            channels.push(
                send_wecom(
                    &self.http,
                    &cfg.wecom.webhook_url,
                    payload,
                    &text,
                    &wecom_md,
                )
                .await,
            );
        }
        if cfg.email.enabled && !cfg.email.to.is_empty() && !cfg.email.smtp_host.is_empty() {
            channels.push(send_email(&cfg.email, &title, &text).await);
        }
        if cfg.sms.enabled && !cfg.sms.phones.is_empty() {
            channels.push(send_sms(&self.http, &cfg.sms, &text).await);
        }
        for url in &cfg.webhooks {
            channels.push(send_generic_webhook(&self.http, url, payload, &text).await);
        }

        let delivered = channels.iter().filter(|c| c.ok).count();
        let status = if delivered > 0 {
            "sent".into()
        } else if channels.is_empty() {
            "no_channel".into()
        } else {
            "failed".into()
        };
        let detail = channels
            .iter()
            .map(|c| format!("{}:{}", c.channel, if c.ok { "ok" } else { "fail" }))
            .collect::<Vec<_>>()
            .join("; ");
        DispatchResult {
            delivered,
            channels,
            status,
            detail,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_min_severity() -> String {
    "medium".into()
}
fn default_smtp_port() -> u16 {
    465
}
fn default_sms_provider() -> String {
    "webhook".into()
}
fn default_cooldown_sec() -> u32 {
    300
}
fn default_weekly_hour() -> u8 {
    0
}
fn default_unack_after() -> u32 {
    60
}
fn default_unack_interval() -> u32 {
    3600
}
fn default_unack_max() -> u32 {
    3
}

fn cooldown_key(payload: &AlertPayload) -> String {
    format!(
        "{}:{}",
        payload.category,
        payload.src_ip.as_deref().unwrap_or("-")
    )
}

fn has_any_channel(cfg: &AlertConfig) -> bool {
    (!cfg.webhooks.is_empty())
        || (cfg.wecom.enabled && !cfg.wecom.webhook_url.is_empty())
        || (cfg.email.enabled && !cfg.email.smtp_host.is_empty() && !cfg.email.to.is_empty())
        || (cfg.sms.enabled && !cfg.sms.phones.is_empty())
}

/// Migrate legacy WeCom webhook into the wecom section.
pub(crate) fn normalize_config(mut cfg: AlertConfig) -> AlertConfig {
    if cfg.wecom.webhook_url.is_empty() {
        let mut wecom_urls: Vec<String> = cfg
            .webhooks
            .iter()
            .filter(|u| u.contains("qyapi.weixin.qq.com"))
            .cloned()
            .collect();
        if let Some(url) = wecom_urls.pop() {
            cfg.wecom.enabled = cfg.wecom.enabled || cfg.enabled;
            cfg.wecom.webhook_url = url;
            cfg.webhooks.retain(|u| !u.contains("qyapi.weixin.qq.com"));
        }
    }
    cfg
}

fn display_field(value: Option<String>) -> String {
    value
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "—".into())
}

fn severity_label(severity: &str) -> String {
    match severity.to_lowercase().as_str() {
        "critical" => "严重".into(),
        "high" => "高危".into(),
        "medium" => "中危".into(),
        "low" => "低危".into(),
        other => other.to_string(),
    }
}

fn action_label(action: &str) -> String {
    match action.to_lowercase().as_str() {
        "block" => "已拦截".into(),
        "log" => "已记录".into(),
        "allow" => "已放行".into(),
        "deny" => "已拒绝".into(),
        "challenge" => "人机挑战".into(),
        other => other.to_string(),
    }
}

fn category_label(category: &str) -> String {
    match category.to_lowercase().as_str() {
        "sqli" => "SQL 注入".into(),
        "xss" => "XSS".into(),
        "rce" => "命令注入".into(),
        "path_traversal" => "路径穿越".into(),
        "ssrf" => "SSRF".into(),
        "webshell" => "Webshell".into(),
        "cc" => "CC 攻击".into(),
        "l3_policy" => "L3 策略".into(),
        "prompt_injection" => "Prompt 注入".into(),
        "bot" => "Bot".into(),
        "anomaly" => "异常行为".into(),
        "custom" => "自定义规则".into(),
        "expert" => "专家规则".into(),
        other => other.to_string(),
    }
}

pub(crate) fn render_template(tpl: &str, payload: &AlertPayload) -> String {
    let mut out = tpl.to_string();
    let pairs: [(&str, String); 14] = [
        ("severity", payload.severity.clone()),
        ("severity_label", severity_label(&payload.severity)),
        ("category", payload.category.clone()),
        ("category_label", category_label(&payload.category)),
        ("action", payload.action.clone()),
        ("action_label", action_label(&payload.action)),
        ("src_ip", display_field(payload.src_ip.clone())),
        ("host", display_field(payload.host.clone())),
        ("uri", display_field(payload.uri.clone())),
        ("message", payload.message.clone()),
        ("site_id", display_field(payload.site_id.clone())),
        (
            "time",
            payload.ts.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ),
        (
            "trace_id",
            payload.trace_id.clone().unwrap_or_else(|| "—".into()),
        ),
        (
            "span_id",
            payload.span_id.clone().unwrap_or_else(|| "—".into()),
        ),
    ];
    for (key, val) in pairs {
        out = out.replace(&format!("${{{key}}}"), &val);
    }
    out
}

#[allow(dead_code)]
fn format_alert_text(payload: &AlertPayload) -> String {
    render_template(
        "[Nerve WAF] 严重度=${severity} 类型=${category} 动作=${action} 源IP=${src_ip} 主机=${host} URI=${uri} 说明=${message}",
        payload,
    )
}

#[allow(dead_code)]
fn format_alert_markdown(payload: &AlertPayload) -> String {
    render_template(
        "## Nerve WAF 安全告警\n\
         > **严重度** <font color=\"warning\">${severity}</font>\n\
         > **攻击类型** ${category}\n\
         > **处置动作** ${action}\n\
         > **源 IP** ${src_ip}\n\
         > **Host** ${host}\n\
         > **URI** ${uri}\n\
         > **说明** ${message}\n\
         > **时间** ${time}",
        payload,
    )
}

async fn send_wecom(
    http: &reqwest::Client,
    url: &str,
    payload: &AlertPayload,
    text: &str,
    markdown: &str,
) -> ChannelResult {
    let body = if url.contains("qyapi.weixin.qq.com") {
        serde_json::json!({
            "msgtype": "markdown",
            "markdown": { "content": markdown }
        })
    } else {
        serde_json::json!({ "text": text, "payload": payload })
    };
    match http.post(url).json(&body).send().await {
        Ok(r) if r.status().is_success() => ChannelResult {
            channel: "wecom".into(),
            ok: true,
            detail: "已发送".into(),
        },
        Ok(r) => ChannelResult {
            channel: "wecom".into(),
            ok: false,
            detail: format!("HTTP {}", r.status()),
        },
        Err(e) => ChannelResult {
            channel: "wecom".into(),
            ok: false,
            detail: e.to_string(),
        },
    }
}

async fn send_generic_webhook(
    http: &reqwest::Client,
    url: &str,
    payload: &AlertPayload,
    text: &str,
) -> ChannelResult {
    let body = serde_json::json!({ "text": text, "payload": payload });
    match http.post(url).json(&body).send().await {
        Ok(r) if r.status().is_success() => ChannelResult {
            channel: "webhook".into(),
            ok: true,
            detail: url.to_string(),
        },
        Ok(r) => ChannelResult {
            channel: "webhook".into(),
            ok: false,
            detail: format!("{} → HTTP {}", url, r.status()),
        },
        Err(e) => ChannelResult {
            channel: "webhook".into(),
            ok: false,
            detail: format!("{} → {}", url, e),
        },
    }
}

async fn send_email(cfg: &EmailConfig, subject: &str, body: &str) -> ChannelResult {
    use lettre::message::header::ContentType;
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::AsyncTransport;
    use lettre::{AsyncSmtpTransport, Message, Tokio1Executor};

    let from: Mailbox = match cfg.from.parse() {
        Ok(m) => m,
        Err(e) => {
            return ChannelResult {
                channel: "email".into(),
                ok: false,
                detail: format!("发件人地址无效: {e}"),
            };
        }
    };

    let mut builder = Message::builder().from(from).subject(subject.to_string());

    for to in &cfg.to {
        if to.trim().is_empty() {
            continue;
        }
        match to.parse() {
            Ok(m) => builder = builder.to(m),
            Err(e) => {
                return ChannelResult {
                    channel: "email".into(),
                    ok: false,
                    detail: format!("收件人 {to} 无效: {e}"),
                };
            }
        }
    }

    let email = match builder
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
    {
        Ok(m) => m,
        Err(e) => {
            return ChannelResult {
                channel: "email".into(),
                ok: false,
                detail: e.to_string(),
            };
        }
    };

    let creds = if cfg.username.is_empty() {
        None
    } else {
        Some(Credentials::new(
            cfg.username.clone(),
            cfg.password.clone(),
        ))
    };

    let mailer: Result<AsyncSmtpTransport<Tokio1Executor>, lettre::transport::smtp::Error> =
        if cfg.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.smtp_host).map(|b| {
                let b = b.port(cfg.smtp_port);
                let b = if let Some(c) = creds.clone() {
                    b.credentials(c)
                } else {
                    b
                };
                b.build()
            })
        } else {
            Ok(
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.smtp_host)
                    .port(cfg.smtp_port)
                    .build(),
            )
        };

    match mailer {
        Ok(m) => match m.send(email).await {
            Ok(_) => ChannelResult {
                channel: "email".into(),
                ok: true,
                detail: format!("已发送至 {}", cfg.to.join(", ")),
            },
            Err(e) => ChannelResult {
                channel: "email".into(),
                ok: false,
                detail: e.to_string(),
            },
        },
        Err(e) => ChannelResult {
            channel: "email".into(),
            ok: false,
            detail: format!("SMTP 连接失败: {e}"),
        },
    }
}

async fn send_sms(http: &reqwest::Client, cfg: &SmsConfig, text: &str) -> ChannelResult {
    if cfg.provider == "aliyun" {
        return ChannelResult {
            channel: "sms".into(),
            ok: false,
            detail: "阿里云短信需配置专用网关，请暂用 Webhook 模式".into(),
        };
    }

    if cfg.webhook_url.is_empty() {
        return ChannelResult {
            channel: "sms".into(),
            ok: false,
            detail: "未配置 SMS Webhook URL".into(),
        };
    }

    let body = serde_json::json!({
        "phones": cfg.phones,
        "content": text,
        "sign": cfg.sign_name,
        "template": cfg.template_code,
    });

    match http.post(&cfg.webhook_url).json(&body).send().await {
        Ok(r) if r.status().is_success() => ChannelResult {
            channel: "sms".into(),
            ok: true,
            detail: format!("已通知 {}", cfg.phones.join(", ")),
        },
        Ok(r) => ChannelResult {
            channel: "sms".into(),
            ok: false,
            detail: format!("HTTP {}", r.status()),
        },
        Err(e) => ChannelResult {
            channel: "sms".into(),
            ok: false,
            detail: e.to_string(),
        },
    }
}

fn severity_rank(s: &str) -> u8 {
    match s.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

impl Default for AlertDispatcher {
    fn default() -> Self {
        Self::new(AlertConfig::default())
    }
}

#[cfg(test)]
mod test;
