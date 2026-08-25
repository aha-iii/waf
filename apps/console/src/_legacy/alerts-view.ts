/** Legacy console module: Alerts view. */
import { apiGet, apiPost, apiPut } from "./api.js";
import { bindPaginationBar, categoryLabel, defaultPaginationState, esc, pageHeader, renderPaginatedSection, showConfirm, switchHtml, type PaginationState } from "./components/ui.js";

type AlertTemplateConfig = {
  title: string;
  body: string;
  wecom_markdown: string;
};

type WeComConfig = { enabled: boolean; webhook_url: string };
type EmailConfig = {
  enabled: boolean;
  smtp_host: string;
  smtp_port: number;
  use_tls: boolean;
  username: string;
  password: string;
  from: string;
  to: string[];
};
type SmsConfig = {
  enabled: boolean;
  provider: string;
  webhook_url: string;
  access_key: string;
  secret_key: string;
  sign_name: string;
  template_code: string;
  phones: string[];
};

type AlertDigestConfig = {
  daily_enabled: boolean;
  daily_hour_utc: number;
  weekly_enabled: boolean;
  weekly_weekday: number;
  weekly_hour_utc: number;
  unack_enabled: boolean;
  unack_after_minutes: number;
  unack_interval_sec: number;
  unack_remind_max: number;
};

type AlertConfig = {
  enabled: boolean;
  webhooks: string[];
  min_severity: string;
  cooldown_sec?: number;
  digest?: AlertDigestConfig;
  categories: string[];
  wecom: WeComConfig;
  email: EmailConfig;
  sms: SmsConfig;
  template: AlertTemplateConfig;
};

type AlertTab = "policy" | "wecom" | "email" | "sms" | "webhook" | "template" | "logs";

type AlertLog = {
  id: string;
  ts: string;
  trigger: string;
  status: string;
  detail: string;
  event_id?: string;
  severity: string;
  category: string;
  action: string;
  src_ip?: string;
  host?: string;
  uri?: string;
  trace_id?: string;
  span_id?: string;
  delivered: number;
  channels?: { channel: string; ok: boolean; detail: string }[];
  acked?: boolean;
  acked_at?: string;
  acked_by?: string;
  remind_count?: number;
};

const SEVERITY_OPTIONS = [
  { value: "low", label: "低危及以上" },
  { value: "medium", label: "中危及以上" },
  { value: "high", label: "高危及以上" },
  { value: "critical", label: "仅严重" },
];

const CATEGORY_PRESETS = [
  "sqli", "xss", "rce", "path_traversal", "ssrf", "webshell", "cc", "l3_policy", "prompt_injection", "bot",
];

const TEMPLATE_VARS = [
  { key: "severity_label", label: "严重级别(中文)" },
  { key: "severity", label: "严重级别(原始)" },
  { key: "category_label", label: "攻击类型(中文)" },
  { key: "category", label: "攻击类型(原始)" },
  { key: "action_label", label: "处置动作(中文)" },
  { key: "action", label: "处置动作(原始)" },
  { key: "src_ip", label: "源 IP" },
  { key: "host", label: "域名" },
  { key: "uri", label: "请求路径" },
  { key: "message", label: "详情" },
  { key: "site_id", label: "防护应用" },
  { key: "time", label: "时间" },
  { key: "trace_id", label: "Trace ID" },
  { key: "span_id", label: "Span ID" },
];

const SEVERITY_LABEL: Record<string, string> = {
  critical: "严重",
  high: "高危",
  medium: "中危",
  low: "低危",
};

const ACTION_LABEL: Record<string, string> = {
  block: "已拦截",
  log: "已记录",
  allow: "已放行",
  deny: "已拒绝",
  challenge: "人机挑战",
};

const PREVIEW_SAMPLE: Record<string, string> = {
  severity: "high",
  category: "webshell",
  action: "block",
  src_ip: "127.0.0.1",
  host: "",
  uri: "/upload?f=<?php eval(base64...",
  message: "webshell",
  site_id: "default",
  time: "2026-07-05 01:38:50 UTC",
  trace_id: "2ea8ac59-9c7d-4c94-b432-6a8550c725e7",
  span_id: "88bfa649-d80a-4830-baff-1b81d3ee7922",
};

const DEFAULT_TEMPLATE: AlertTemplateConfig = {
  title: "[WAF告警] ${severity_label} · ${category_label} · ${action_label}",
  body: `━━━━━━━━━━━━━━━━━━━━━━
 Nerve WAF · 安全告警
━━━━━━━━━━━━━━━━━━━━━━

【事件概要】
  严重级别：\${severity_label} (\${severity})
  攻击类型：\${category_label}
  处置动作：\${action_label}

【请求信息】
  源 IP   ：\${src_ip}
  防护应用：\${site_id}
  域名    ：\${host}
  请求路径：\${uri}
  详情    ：\${message}

【时间】\${time}

【链路追踪】
  Trace ID：\${trace_id}
  Span ID ：\${span_id}`,
  wecom_markdown: `## 🛡️ WAF 安全告警

> **严重级别** <font color="warning">\${severity_label}</font>　**处置** <font color="comment">\${action_label}</font>
> **攻击类型** \${category_label}

---

> **源 IP** \${src_ip}　**应用** \${site_id}
> **域名** \${host}　**时间** \${time}

> **请求路径**
> \`\${uri}\`

> **规则说明** \${message}

---

> **链路追踪**
> Trace \`\${trace_id}\`
> Span \`\${span_id}\``,
};

const NAV_ITEMS: { id: AlertTab; icon: string; label: string; desc: string }[] = [
  { id: "policy", icon: "⚙", label: "通知策略", desc: "触发条件与告警范围" },
  { id: "wecom", icon: "💬", label: "企业微信", desc: "群机器人 Webhook" },
  { id: "email", icon: "✉", label: "邮件", desc: "SMTP 邮件通道" },
  { id: "sms", icon: "📱", label: "短信", desc: "HTTP 短信网关" },
  { id: "webhook", icon: "🔗", label: "Webhook", desc: "自定义 HTTP 回调" },
  { id: "template", icon: "📝", label: "告警模板", desc: "消息标题与正文格式" },
  { id: "logs", icon: "📋", label: "告警记录", desc: "推送历史 · 审计追溯" },
];

function splitLines(s: string): string[] {
  return s.split(/[\n,;]/).map((x) => x.trim()).filter(Boolean);
}

function renderPreview(tpl: string): string {
  const sample: Record<string, string> = {
    ...PREVIEW_SAMPLE,
    severity_label: SEVERITY_LABEL[PREVIEW_SAMPLE.severity] ?? PREVIEW_SAMPLE.severity,
    category_label: categoryLabel(PREVIEW_SAMPLE.category),
    action_label: ACTION_LABEL[PREVIEW_SAMPLE.action] ?? PREVIEW_SAMPLE.action,
    host: PREVIEW_SAMPLE.host?.trim() ? PREVIEW_SAMPLE.host : "—",
  };
  return tpl.replace(/\$\{(\w+)\}/g, (_, k) => sample[k] ?? "—");
}

function channelReady(cfg: AlertConfig, id: AlertTab): boolean {
  if (id === "wecom") return cfg.wecom.enabled && !!cfg.wecom.webhook_url.trim();
  if (id === "email") return cfg.email.enabled && !!cfg.email.smtp_host.trim() && cfg.email.to.length > 0;
  if (id === "sms") return cfg.sms.enabled && cfg.sms.phones.length > 0;
  if (id === "webhook") return (cfg.webhooks ?? []).length > 0;
  return true;
}

export async function renderAlertsView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const res = await apiGet<{ config: AlertConfig }>("/api/alerts/config");
  let cfg = normalizeConfig(res.config);
  let activeTab: AlertTab = "policy";

  function render(): void {
    root.innerHTML = `
      ${pageHeader("告警配置", "")}
      <div class="ant-card alert-console">
        <div class="alert-console-toolbar">
          ${switchHtml("alert-enabled", cfg.enabled, "启用告警")}
          <span class="alert-toolbar-spacer"></span>
          <button type="button" class="ant-btn" id="btn-test-alert">发送测试</button>
          <button type="button" class="ant-btn ant-btn-primary" id="btn-save-alert">发布</button>
        </div>
        <div class="alert-console-body">
          <nav class="alert-console-nav" id="alert-nav">
            ${NAV_ITEMS.map(
              (item) => `<button type="button" class="alert-nav-item${activeTab === item.id ? " active" : ""}" data-tab="${item.id}">
                <span class="alert-nav-icon">${item.icon}</span>
                <span class="alert-nav-text">
                  <strong>${item.label}</strong>
                  <span class="muted small">${item.desc}</span>
                </span>
                ${item.id !== "policy" && item.id !== "template" && item.id !== "logs"
                  ? `<span class="alert-nav-dot ${channelReady(cfg, item.id) ? "on" : ""}"></span>`
                  : ""}
              </button>`,
            ).join("")}
          </nav>
          <div class="alert-console-panel" id="alert-panel">${renderPanel(activeTab, cfg)}</div>
        </div>
        <div class="alert-console-footer muted small" id="alert-test-result"></div>
      </div>`;

    bindNav(root);
    bindPanel(root);
    bindSave(root, toast, () => cfg, (next) => {
      cfg = next;
      render();
    });
    bindTest(root, toast, () => cfg, (next) => {
      cfg = next;
    });
    root.querySelector("#btn-refresh-logs")?.addEventListener("click", () => void loadAlertLogs(root, toast));
    if (activeTab === "logs") void loadAlertLogs(root, toast);
  }

  function syncFromForm(): void {
    cfg = collectConfig(root, cfg);
  }

  function bindNav(el: HTMLElement): void {
    el.querySelectorAll<HTMLButtonElement>(".alert-nav-item").forEach((btn) => {
      btn.onclick = () => {
        syncFromForm();
        activeTab = (btn.dataset.tab as AlertTab) || "policy";
        render();
      };
    });
  }

  function bindPanel(el: HTMLElement): void {
    el.querySelectorAll<HTMLButtonElement>("#alert-cat-chips .chip-btn").forEach((btn) => {
      btn.onclick = () => {
        btn.classList.toggle("active");
        const hidden = el.querySelector("#alert-categories-val") as HTMLInputElement;
        hidden.value = [...el.querySelectorAll<HTMLButtonElement>("#alert-cat-chips .chip-btn.active")]
          .map((b) => b.dataset.cat!)
          .filter(Boolean)
          .join(",");
      };
    });

    el.querySelectorAll<HTMLButtonElement>(".tpl-var-btn").forEach((btn) => {
      btn.onclick = () => {
        const targetId = btn.dataset.target!;
        const input = el.querySelector(`#${targetId}`) as HTMLTextAreaElement | HTMLInputElement | null;
        if (!input) return;
        const token = `\${${btn.dataset.var}}`;
        const pos = input.selectionStart ?? input.value.length;
        input.value = input.value.slice(0, pos) + token + input.value.slice(pos);
        input.focus();
        updateTemplatePreview(el);
      };
    });

    ["tpl-title", "tpl-body", "tpl-wecom"].forEach((id) => {
      el.querySelector(`#${id}`)?.addEventListener("input", () => updateTemplatePreview(el));
    });
    updateTemplatePreview(el);
    el.querySelector("#tpl-reset")?.addEventListener("click", () => {
      (el.querySelector("#tpl-title") as HTMLInputElement).value = DEFAULT_TEMPLATE.title;
      (el.querySelector("#tpl-body") as HTMLTextAreaElement).value = DEFAULT_TEMPLATE.body;
      (el.querySelector("#tpl-wecom") as HTMLTextAreaElement).value = DEFAULT_TEMPLATE.wecom_markdown;
      updateTemplatePreview(el);
    });

    const runDigest = async (kind: string) => {
      try {
        const r = await apiPost<{ ok: boolean; detail?: string; error?: string }>("/api/alerts/digest/run", { kind });
        toast(r.ok ? (r.detail ?? "已执行") : (r.error ?? "执行失败"), r.ok);
      } catch (e) {
        toast(String(e), false);
      }
    };
    el.querySelector("#btn-run-daily")?.addEventListener("click", () => void runDigest("daily"));
    el.querySelector("#btn-run-weekly")?.addEventListener("click", () => void runDigest("weekly"));
    el.querySelector("#btn-run-unack")?.addEventListener("click", () => void runDigest("unack"));
  }

  render();
}

function renderPanel(tab: AlertTab, cfg: AlertConfig): string {
  switch (tab) {
    case "policy":
      return renderPolicyPanel(cfg);
    case "wecom":
      return renderWecomPanel(cfg.wecom);
    case "email":
      return renderEmailPanel(cfg.email);
    case "sms":
      return renderSmsPanel(cfg.sms);
    case "webhook":
      return renderWebhookPanel(cfg.webhooks ?? []);
    case "template":
      return renderTemplatePanel(cfg.template ?? DEFAULT_TEMPLATE);
    case "logs":
      return renderLogsPanelShell();
    default:
      return "";
  }
}

function defaultDigest(): AlertDigestConfig {
  return {
    daily_enabled: true,
    daily_hour_utc: 0,
    weekly_enabled: true,
    weekly_weekday: 0,
    weekly_hour_utc: 0,
    unack_enabled: true,
    unack_after_minutes: 60,
    unack_interval_sec: 3600,
    unack_remind_max: 3,
  };
}

function renderPolicyPanel(cfg: AlertConfig): string {
  const d = cfg.digest ?? defaultDigest();
  return `
    <div class="alert-panel-head"><h3>通知策略</h3></div>
    <div class="alert-panel-body form-stack">
      <label class="ant-form-label">最低严重级别</label>
      <select class="ant-input ant-input-wide" id="alert-severity">
        ${SEVERITY_OPTIONS.map(
          (o) => `<option value="${o.value}" ${cfg.min_severity === o.value ? "selected" : ""}>${o.label}</option>`,
        ).join("")}
      </select>
      <label class="ant-form-label">告警事件类型</label>
      <input type="hidden" id="alert-categories-val" value="${esc((cfg.categories ?? []).join(","))}" />
      <div class="chip-row" id="alert-cat-chips">
        ${CATEGORY_PRESETS.map((c) => {
          const on = (cfg.categories ?? []).includes(c);
          return `<button type="button" class="chip-btn${on ? " active" : ""}" data-cat="${c}">${categoryLabel(c)}</button>`;
        }).join("")}
      </div>
      <p class="muted small">仅命中所选类型的安全事件会推送通知</p>
      <label class="ant-form-label">告警冷却（秒）</label>
      <input class="ant-input ant-input-wide" id="alert-cooldown" type="number" min="0" step="60"
        value="${cfg.cooldown_sec ?? 300}" />
      <p class="muted small">同一攻击类型 + 源 IP 在冷却期内只推送一次（0 = 不限制）</p>
      <div class="ant-form-section-title">定时摘要 JOB</div>
      ${switchHtml("digest-daily", d.daily_enabled, "每日安全摘要")}
      <label class="ant-form-label">每日摘要时间（UTC 小时，0 ≈ 北京时间 08:00）</label>
      <input class="ant-input ant-input-wide" id="digest-daily-hour" type="number" min="0" max="23" value="${d.daily_hour_utc}" />
      ${switchHtml("digest-weekly", d.weekly_enabled, "每周安全摘要")}
      <div class="route-form-grid-2">
        <div><label class="ant-form-label">周报星期（0=周一）</label>
          <input class="ant-input ant-input-wide" id="digest-weekly-day" type="number" min="0" max="6" value="${d.weekly_weekday}" /></div>
        <div><label class="ant-form-label">周报 UTC 小时</label>
          <input class="ant-input ant-input-wide" id="digest-weekly-hour" type="number" min="0" max="23" value="${d.weekly_hour_utc}" /></div>
      </div>
      ${switchHtml("digest-unack", d.unack_enabled, "未 ACK 告警复查")}
      <div class="route-form-grid-2">
        <div><label class="ant-form-label">超过 N 分钟未 ACK 则提醒</label>
          <input class="ant-input ant-input-wide" id="digest-unack-after" type="number" min="5" value="${d.unack_after_minutes}" /></div>
        <div><label class="ant-form-label">复查间隔（秒）</label>
          <input class="ant-input ant-input-wide" id="digest-unack-interval" type="number" min="300" value="${d.unack_interval_sec}" /></div>
      </div>
      <label class="ant-form-label">单条最多提醒次数</label>
      <input class="ant-input ant-input-wide" id="digest-unack-max" type="number" min="1" max="10" value="${d.unack_remind_max}" />
      <div class="chip-row">
        <button type="button" class="ant-btn ant-btn-sm" id="btn-run-daily">立即发送日报</button>
        <button type="button" class="ant-btn ant-btn-sm" id="btn-run-weekly">立即发送周报</button>
        <button type="button" class="ant-btn ant-btn-sm" id="btn-run-unack">立即复查未 ACK</button>
      </div>
    </div>`;
}

const TRIGGER_LABEL: Record<string, string> = {
  realtime: "实时检测",
  test: "通道测试",
  ingest: "事件入库",
  digest_daily: "日报 JOB",
  digest_weekly: "周报 JOB",
  digest_unack: "未ACK复查",
};

const STATUS_LABEL: Record<string, string> = {
  sent: "已推送",
  suppressed: "冷却抑制",
  skipped: "策略跳过",
  failed: "发送失败",
  no_channel: "无通道",
};

function statusTag(status: string): string {
  const cls =
    status === "sent"
      ? "ant-tag-green"
      : status === "suppressed" || status === "skipped"
        ? "ant-tag-orange"
        : status === "failed"
          ? "ant-tag-red"
          : "ant-tag-default";
  return `<span class="ant-tag ${cls}">${esc(STATUS_LABEL[status] ?? status)}</span>`;
}

function renderLogsPanelShell(): string {
  return `
    <div class="alert-panel-head">
      <div class="alert-panel-head-row">
        <div>
          <h3>告警记录</h3>
          <p class="muted">每次推送尝试均落库（含冷却抑制、策略跳过），便于审计与排障</p>
        </div>
        <button type="button" class="ant-btn ant-btn-sm" id="btn-refresh-logs">刷新</button>
      </div>
    </div>
    <div class="alert-panel-body alert-logs-body" id="alert-logs-body">
      <p class="muted">加载中…</p>
    </div>`;
}

const logsPage: PaginationState = defaultPaginationState();
let cachedAlertLogs: AlertLog[] = [];
let cachedAlertLogsTotal = 0;

function bindAckButtons(root: HTMLElement, toast: (m: string, ok?: boolean) => void): void {
  root.querySelectorAll<HTMLButtonElement>(".btn-ack-log").forEach((btn) => {
    btn.onclick = async () => {
      try {
        await apiPost(`/api/alerts/logs/${btn.dataset.id}/ack`, { acked_by: "console" });
        toast("已确认告警");
        await loadAlertLogs(root, toast);
      } catch (e) {
        toast(String(e), false);
      }
    };
  });
}

function paintAlertLogsTable(root: HTMLElement, toast: (m: string, ok?: boolean) => void): void {
  const body = root.querySelector("#alert-logs-body");
  if (!body) return;
  if (!cachedAlertLogs.length) {
    body.innerHTML = `<p class="muted">暂无告警记录</p>`;
    return;
  }
  const section = renderPaginatedSection("alert-logs", cachedAlertLogs, logsPage, (pageLogs) => {
    const rows = pageLogs
      .map(
        (l) => `<tr>
          <td class="small">${esc(l.ts.replace("T", " ").replace("+00:00", " UTC"))}</td>
          <td>${statusTag(l.status)}</td>
          <td>${esc(TRIGGER_LABEL[l.trigger] ?? l.trigger)}</td>
          <td>${esc(categoryLabel(l.category))}</td>
          <td>${esc(l.src_ip ?? "-")}</td>
          <td class="small trace-cell" title="${esc(l.trace_id ?? "")}">${esc((l.trace_id ?? "-").slice(0, 8))}</td>
          <td>${l.delivered}</td>
          <td>${l.acked ? '<span class="ant-tag ant-tag-green">已ACK</span>' : l.status === "sent" && (l.trigger === "realtime" || l.trigger === "ingest") ? `<button type="button" class="ant-btn ant-btn-sm ant-btn-link btn-ack-log" data-id="${esc(l.id)}">确认</button>` : "-"}</td>
          <td class="small muted">${esc(l.detail)}</td>
        </tr>`,
      )
      .join("");
    return `
      <p class="muted small">共 ${cachedAlertLogsTotal} 条 · 展示最近 ${cachedAlertLogs.length} 条</p>
      <div class="ant-table-wrapper">
        <table class="ant-table alert-logs-table">
          <thead><tr>
            <th>时间</th><th>状态</th><th>来源</th><th>类型</th><th>源 IP</th><th>Trace</th><th>通道</th><th>ACK</th><th>说明</th>
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>`;
  });
  logsPage.page = section.page;
  body.innerHTML = section.html;
  bindPaginationBar(body, "alert-logs", logsPage, cachedAlertLogs.length, (next) => {
    logsPage.page = next.page;
    logsPage.pageSize = next.pageSize;
    paintAlertLogsTable(root, toast);
  });
  bindAckButtons(body, toast);
}

async function loadAlertLogs(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const body = root.querySelector("#alert-logs-body");
  if (!body) return;
  try {
    const r = await apiGet<{ total: number; logs: AlertLog[] }>("/api/alerts/logs?limit=100");
    cachedAlertLogs = r.logs ?? [];
    cachedAlertLogsTotal = r.total ?? 0;
    logsPage.page = 1;
    paintAlertLogsTable(root, toast);
  } catch (e) {
    body.innerHTML = `<p class="muted">${esc(String(e))}</p>`;
    toast(String(e), false);
  }
}

function renderWecomPanel(wecom: WeComConfig): string {
  return `
    <div class="alert-panel-head"><h3>企业微信</h3></div>
    <div class="alert-panel-body form-stack">
      ${switchHtml("wecom-enabled", wecom.enabled, "启用此通道")}
      <label class="ant-form-label">机器人 Webhook URL</label>
      <input class="ant-input ant-input-wide" id="wecom-url" type="url"
        placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."
        value="${esc(wecom.webhook_url)}" />
    </div>`;
}

function renderEmailPanel(email: EmailConfig): string {
  return `
    <div class="alert-panel-head"><h3>邮件通知</h3></div>
    <div class="alert-panel-body form-stack">
      ${switchHtml("email-enabled", email.enabled, "启用此通道")}
      <div class="route-form-grid-2">
        <div>
          <label class="ant-form-label">SMTP 服务器</label>
          <input class="ant-input ant-input-wide" id="email-host" placeholder="smtp.example.com" value="${esc(email.smtp_host)}" />
        </div>
        <div>
          <label class="ant-form-label">端口</label>
          <input class="ant-input ant-input-wide" id="email-port" type="number" value="${email.smtp_port || 465}" />
        </div>
      </div>
      <label class="switch-wrap"><span>TLS/SSL</span>
        <span class="switch"><input type="checkbox" id="email-tls" ${email.use_tls ? "checked" : ""} /><span></span></span>
      </label>
      <div class="route-form-grid-2">
        <div>
          <label class="ant-form-label">用户名</label>
          <input class="ant-input ant-input-wide" id="email-user" autocomplete="off" value="${esc(email.username)}" />
        </div>
        <div>
          <label class="ant-form-label">密码 / 授权码</label>
          <input class="ant-input ant-input-wide" id="email-pass" type="password" autocomplete="new-password" value="${esc(email.password)}" />
        </div>
      </div>
      <label class="ant-form-label">发件人</label>
      <input class="ant-input ant-input-wide" id="email-from" placeholder="waf@example.com" value="${esc(email.from)}" />
      <label class="ant-form-label">收件人</label>
      <textarea class="ant-input ant-input-wide" id="email-to" rows="3" placeholder="每行一个邮箱">${esc((email.to ?? []).join("\n"))}</textarea>
    </div>`;
}

function renderSmsPanel(sms: SmsConfig): string {
  return `
    <div class="alert-panel-head"><h3>短信通知</h3></div>
    <div class="alert-panel-body form-stack">
      ${switchHtml("sms-enabled", sms.enabled, "启用此通道")}
      <label class="ant-form-label">网关 Webhook URL</label>
      <input class="ant-input ant-input-wide" id="sms-url" type="url" placeholder="https://sms-gateway/internal/send" value="${esc(sms.webhook_url)}" />
      <div class="route-form-grid-2">
        <div>
          <label class="ant-form-label">短信签名</label>
          <input class="ant-input ant-input-wide" id="sms-sign" value="${esc(sms.sign_name)}" placeholder="NerveWAF" />
        </div>
        <div>
          <label class="ant-form-label">模板 ID（网关侧）</label>
          <input class="ant-input ant-input-wide" id="sms-template" value="${esc(sms.template_code)}" />
        </div>
      </div>
      <label class="ant-form-label">接收手机号</label>
      <textarea class="ant-input ant-input-wide" id="sms-phones" rows="3" placeholder="每行一个">${esc((sms.phones ?? []).join("\n"))}</textarea>
    </div>`;
}

function renderWebhookPanel(webhooks: string[]): string {
  return `
    <div class="alert-panel-head"><h3>Webhook</h3></div>
    <div class="alert-panel-body form-stack">
      <label class="ant-form-label">Webhook URL</label>
      <textarea class="ant-input ant-input-wide" id="alert-webhooks" rows="4" placeholder="每行一个 URL">${esc(webhooks.join("\n"))}</textarea>
    </div>`;
}

function renderTemplatePanel(tpl: AlertTemplateConfig): string {
  const vars = TEMPLATE_VARS.map(
    (v) => `<button type="button" class="chip-btn tpl-var-btn" data-var="${v.key}" data-target="tpl-body" title="插入到正文">${v.label} \${${v.key}}</button>`,
  ).join("");
  return `
    <div class="alert-panel-head"><h3>告警模板</h3></div>
    <div class="alert-panel-body">
      <div class="alert-template-layout">
        <div class="form-stack">
          <label class="ant-form-label">邮件主题 / 告警标题</label>
          <input class="ant-input ant-input-wide" id="tpl-title" value="${esc(tpl.title)}" />
          <label class="ant-form-label">正文模板（邮件 · 短信 · Webhook）</label>
          <textarea class="ant-input ant-input-wide tpl-editor" id="tpl-body" rows="8">${esc(tpl.body)}</textarea>
          <label class="ant-form-label">企业微信 Markdown</label>
          <textarea class="ant-input ant-input-wide tpl-editor" id="tpl-wecom" rows="8">${esc(tpl.wecom_markdown)}</textarea>
          <label class="ant-form-label">插入变量</label>
          <div class="chip-row">${vars}</div>
          <button type="button" class="ant-btn ant-btn-sm" id="tpl-reset">恢复默认模板</button>
        </div>
        <div class="alert-template-preview">
          <div class="alert-preview-head">预览效果</div>
          <div class="alert-preview-block">
            <div class="muted small">邮件主题</div>
            <div id="preview-title" class="alert-preview-title"></div>
          </div>
          <div class="alert-preview-block">
            <div class="muted small">正文</div>
            <pre id="preview-body" class="alert-preview-pre"></pre>
          </div>
          <div class="alert-preview-block">
            <div class="muted small">企业微信</div>
            <pre id="preview-wecom" class="alert-preview-pre"></pre>
          </div>
        </div>
      </div>
    </div>`;
}

function updateTemplatePreview(root: HTMLElement): void {
  const titleEl = root.querySelector("#tpl-title") as HTMLInputElement | null;
  if (!titleEl) return;
  const title = titleEl.value;
  const body = (root.querySelector("#tpl-body") as HTMLTextAreaElement).value;
  const wecom = (root.querySelector("#tpl-wecom") as HTMLTextAreaElement).value;
  root.querySelector("#preview-title")!.textContent = renderPreview(title);
  root.querySelector("#preview-body")!.textContent = renderPreview(body);
  root.querySelector("#preview-wecom")!.textContent = renderPreview(wecom);
}

function normalizeConfig(cfg: AlertConfig): AlertConfig {
  const wecom = cfg.wecom ?? { enabled: false, webhook_url: "" };
  const webhooks = [...(cfg.webhooks ?? [])];
  let merged = {
    ...cfg,
    wecom,
    email: cfg.email ?? defaultEmail(),
    sms: cfg.sms ?? defaultSms(),
    template: cfg.template ?? DEFAULT_TEMPLATE,
    cooldown_sec: cfg.cooldown_sec ?? 300,
    digest: { ...defaultDigest(), ...(cfg.digest ?? {}) },
  };
  if (!wecom.webhook_url && webhooks.some((u) => u.includes("qyapi.weixin.qq.com"))) {
    const url = webhooks.find((u) => u.includes("qyapi.weixin.qq.com"))!;
    merged = {
      ...merged,
      wecom: { enabled: true, webhook_url: url },
      webhooks: webhooks.filter((u) => !u.includes("qyapi.weixin.qq.com")),
    };
  }
  return merged;
}

function defaultEmail(): EmailConfig {
  return { enabled: false, smtp_host: "", smtp_port: 465, use_tls: true, username: "", password: "", from: "", to: [] };
}

function defaultSms(): SmsConfig {
  return { enabled: false, provider: "webhook", webhook_url: "", access_key: "", secret_key: "", sign_name: "", template_code: "", phones: [] };
}

function readChecked(root: HTMLElement, sel: string, prev: boolean): boolean {
  const el = root.querySelector(sel) as HTMLInputElement | null;
  return el?.checked ?? prev;
}

function readValue(root: HTMLElement, sel: string, prev: string): string {
  const el = root.querySelector(sel) as HTMLInputElement | HTMLTextAreaElement | null;
  return el?.value ?? prev;
}

function collectConfig(root: HTMLElement, prev: AlertConfig): AlertConfig {
  const catEl = root.querySelector("#alert-categories-val") as HTMLInputElement | null;
  const categories = catEl
    ? catEl.value.split(",").map((s) => s.trim()).filter(Boolean)
    : (prev.categories ?? []);
  const webhookEl = root.querySelector("#alert-webhooks") as HTMLTextAreaElement | null;
  const email = prev.email ?? defaultEmail();
  const sms = prev.sms ?? defaultSms();
  const template = prev.template ?? DEFAULT_TEMPLATE;
  const digest = prev.digest ?? defaultDigest();

  return {
    enabled: readChecked(root, "#alert-enabled", prev.enabled),
    min_severity: readValue(root, "#alert-severity", prev.min_severity ?? "medium"),
    cooldown_sec: Math.max(0, Number(readValue(root, "#alert-cooldown", String(prev.cooldown_sec ?? 300))) || 0),
    digest: {
      daily_enabled: readChecked(root, "#digest-daily", digest.daily_enabled),
      daily_hour_utc: Math.min(23, Math.max(0, Number(readValue(root, "#digest-daily-hour", String(digest.daily_hour_utc))) || 0)),
      weekly_enabled: readChecked(root, "#digest-weekly", digest.weekly_enabled),
      weekly_weekday: Math.min(6, Math.max(0, Number(readValue(root, "#digest-weekly-day", String(digest.weekly_weekday))) || 0)),
      weekly_hour_utc: Math.min(23, Math.max(0, Number(readValue(root, "#digest-weekly-hour", String(digest.weekly_hour_utc))) || 0)),
      unack_enabled: readChecked(root, "#digest-unack", digest.unack_enabled),
      unack_after_minutes: Math.max(5, Number(readValue(root, "#digest-unack-after", String(digest.unack_after_minutes))) || 60),
      unack_interval_sec: Math.max(300, Number(readValue(root, "#digest-unack-interval", String(digest.unack_interval_sec))) || 3600),
      unack_remind_max: Math.min(10, Math.max(1, Number(readValue(root, "#digest-unack-max", String(digest.unack_remind_max))) || 3)),
    },
    categories,
    webhooks: webhookEl ? splitLines(webhookEl.value) : (prev.webhooks ?? []),
    wecom: {
      enabled: readChecked(root, "#wecom-enabled", prev.wecom?.enabled ?? false),
      webhook_url: readValue(root, "#wecom-url", prev.wecom?.webhook_url ?? "").trim(),
    },
    email: {
      enabled: readChecked(root, "#email-enabled", email.enabled),
      smtp_host: readValue(root, "#email-host", email.smtp_host).trim(),
      smtp_port: Number(readValue(root, "#email-port", String(email.smtp_port || 465))) || 465,
      use_tls: readChecked(root, "#email-tls", email.use_tls),
      username: readValue(root, "#email-user", email.username).trim(),
      password: readValue(root, "#email-pass", email.password),
      from: readValue(root, "#email-from", email.from).trim(),
      to: root.querySelector("#email-to")
        ? splitLines(readValue(root, "#email-to", ""))
        : (email.to ?? []),
    },
    sms: {
      enabled: readChecked(root, "#sms-enabled", sms.enabled),
      provider: sms.provider || "webhook",
      webhook_url: readValue(root, "#sms-url", sms.webhook_url).trim(),
      access_key: sms.access_key,
      secret_key: sms.secret_key,
      sign_name: readValue(root, "#sms-sign", sms.sign_name).trim(),
      template_code: readValue(root, "#sms-template", sms.template_code).trim(),
      phones: root.querySelector("#sms-phones")
        ? splitLines(readValue(root, "#sms-phones", ""))
        : (sms.phones ?? []),
    },
    template: {
      title: readValue(root, "#tpl-title", template.title),
      body: readValue(root, "#tpl-body", template.body),
      wecom_markdown: readValue(root, "#tpl-wecom", template.wecom_markdown),
    },
  };
}

function bindSave(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
  prev: () => AlertConfig,
  onSaved: (cfg: AlertConfig) => void,
): void {
  root.querySelector("#btn-save-alert")?.addEventListener("click", async () => {
    const ok = await showConfirm({
      title: "发布告警配置",
      message: "确认发布当前告警配置？\n\n发布后将立即生效，命中安全事件时按此配置推送通知。",
      okText: "确认发布",
    });
    if (!ok) return;

    const btn = root.querySelector("#btn-save-alert") as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "发布中…";
    try {
      const next = collectConfig(root, prev());
      await apiPut("/api/alerts/config", next);
      onSaved(next);
      toast("告警配置已发布并生效");
    } catch (e) {
      toast(String(e), false);
    } finally {
      btn.disabled = false;
      btn.textContent = "发布";
    }
  });
}

function bindTest(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
  prev: () => AlertConfig,
  onSaved: (cfg: AlertConfig) => void,
): void {
  root.querySelector("#btn-test-alert")?.addEventListener("click", async () => {
    const ok = await showConfirm({
      title: "发送测试告警",
      message: "确认发送测试告警？\n\n将先发布当前配置，再向已启用的通知通道发送一条测试消息。",
      okText: "发送测试",
    });
    if (!ok) return;

    const resultEl = root.querySelector("#alert-test-result")!;
    const btn = root.querySelector("#btn-test-alert") as HTMLButtonElement;
    btn.disabled = true;
    resultEl.textContent = "发送中…";
    try {
      const next = collectConfig(root, prev());
      await apiPut("/api/alerts/config", next);
      onSaved(next);
      const r = await apiPost<{ ok: boolean; delivered: number; channels: { channel: string; ok: boolean; detail: string }[] }>("/api/alerts/test");
      const names: Record<string, string> = { wecom: "企业微信", email: "邮件", sms: "短信", webhook: "Webhook" };
      const lines = (r.channels ?? []).map((c) => `${names[c.channel] ?? c.channel}: ${c.ok ? "成功" : "失败"} — ${c.detail}`);
      resultEl.innerHTML = lines.length ? lines.map((l) => esc(l)).join(" · ") : "未配置任何通知通道";
      toast(r.ok ? `测试告警已发送 (${r.delivered} 个通道)` : "部分或全部通道发送失败", r.ok);
    } catch (e) {
      resultEl.textContent = String(e);
      toast(String(e), false);
    } finally {
      btn.disabled = false;
    }
  });
}
