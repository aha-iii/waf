/** Console page: alert channel and push-rule configuration. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NervePageHeader,
  NervePagination,
  NerveSelect,
  NerveSwitch,
  NerveTable,
  NerveTableActions,
  NerveTag,
  NerveTextarea,
  NERVE_COL_ACTIONS,
  defaultPaginationState,
  nerveConfirm,
  paginate,
} from "@nerve/ui";
import { apiGet, apiPost, apiPut } from "../api";
import { categoryLabel } from "../lib/labels";
import { useToast } from "../hooks/useToast";

type AlertTemplateConfig = { title: string; body: string; wecom_markdown: string };
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
  severity: string;
  category: string;
  src_ip?: string;
  trace_id?: string;
  delivered: number;
  acked?: boolean;
  ack_reason?: string;
};

const SEVERITY_OPTIONS = [
  { value: "low", label: "低危及以上" },
  { value: "medium", label: "中危及以上" },
  { value: "high", label: "高危及以上" },
  { value: "critical", label: "仅严重" },
];

const NAV_ITEMS: { id: AlertTab; icon: string; label: string; desc: string }[] = [
  { id: "policy", icon: "⚙", label: "通知策略", desc: "触发条件与告警范围" },
  { id: "wecom", icon: "💬", label: "企业微信", desc: "群机器人 Webhook" },
  { id: "email", icon: "✉", label: "邮件", desc: "SMTP 邮件通道" },
  { id: "sms", icon: "📱", label: "短信", desc: "HTTP 短信网关" },
  { id: "webhook", icon: "🔗", label: "Webhook", desc: "自定义 HTTP 回调" },
  { id: "template", icon: "📝", label: "告警模板", desc: "消息标题与正文格式" },
  { id: "logs", icon: "📋", label: "告警记录", desc: "推送历史 · 审计追溯" },
];

const DEFAULT_TEMPLATE: AlertTemplateConfig = {
  title: "[Nerve Alert] ${severity_label} · ${category_label} · ${action_label}",
  body: `━━━━━━━━━━━━━━━━━━━━━━
Nerve Alert · 安全告警
━━━━━━━━━━━━━━━━━━━━━━

【事件概要】
  严重级别：\${severity_label}
  攻击类型：\${category_label}
  处置动作：\${action_label}

【请求信息】
  源　　IP：\${src_ip}
  防护应用：\${site_id}
  域　　名：\${host}
  请求路径：\${uri}
  规则说明：\${message}

【时间】\${time}

【链路追踪】
  Trace：\${trace_id}
  Span ：\${span_id}`,
  wecom_markdown: `## Nerve Alert · 安全告警

> **严重级别：** <font color="warning">\${severity_label}</font>
> **攻击类型：** \${category_label}
> **处置动作：** \${action_label}

---

> **源　　IP：** \${src_ip}
> **防护应用：** \${site_id}
> **域　　名：** \${host}
> **时　　间：** \${time}

> **请求路径：** \`\${uri}\`
> **规则说明：** \${message}

---

> **链路追踪**
> Trace：<font color="comment">\${trace_id}</font>
> Span：　<font color="comment">\${span_id}</font>`,
};

const TRIGGER_LABEL: Record<string, string> = {
  realtime: "实时检测",
  test: "通道测试",
  ingest: "事件接入",
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

function splitLines(s: string): string[] {
  return s.split(/[\n,;]/).map((x) => x.trim()).filter(Boolean);
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

function defaultEmail(): EmailConfig {
  return { enabled: false, smtp_host: "", smtp_port: 465, use_tls: true, username: "", password: "", from: "", to: [] };
}

function defaultSms(): SmsConfig {
  return {
    enabled: false,
    provider: "webhook",
    webhook_url: "",
    access_key: "",
    secret_key: "",
    sign_name: "",
    template_code: "",
    phones: [],
  };
}

function normalizeConfig(cfg: AlertConfig): AlertConfig {
  const wecom = cfg.wecom ?? { enabled: false, webhook_url: "" };
  const webhooks = [...(cfg.webhooks ?? [])];
  let merged: AlertConfig = {
    ...cfg,
    // Any rule hit with block can push alerts; no longer filter by type allowlist
    categories: [],
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

function channelReady(cfg: AlertConfig, id: AlertTab): boolean {
  if (id === "wecom") return cfg.wecom.enabled && !!cfg.wecom.webhook_url.trim();
  if (id === "email") return cfg.email.enabled && !!cfg.email.smtp_host.trim() && cfg.email.to.length > 0;
  if (id === "sms") return cfg.sms.enabled && cfg.sms.phones.length > 0;
  if (id === "webhook") return (cfg.webhooks ?? []).length > 0;
  return true;
}

function statusTone(status: string): "green" | "orange" | "red" | "default" {
  if (status === "sent") return "green";
  if (status === "suppressed" || status === "skipped") return "orange";
  if (status === "failed") return "red";
  return "default";
}

export function AlertsPage(): JSX.Element {
  const toast = useToast();
  const [cfg, setCfg] = useState<AlertConfig>(() => normalizeConfig({ enabled: false, webhooks: [], min_severity: "medium", categories: [], wecom: { enabled: false, webhook_url: "" }, email: defaultEmail(), sms: defaultSms(), template: DEFAULT_TEMPLATE }));
  const [activeTab, setActiveTab] = useState<AlertTab>("policy");
  const [testResult, setTestResult] = useState("");
  const [logs, setLogs] = useState<AlertLog[]>([]);
  const [logsTotal, setLogsTotal] = useState(0);
  const [logsPage, setLogsPage] = useState(defaultPaginationState());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [ackTarget, setAckTarget] = useState<AlertLog | null>(null);
  const [ackReason, setAckReason] = useState("");
  const [acking, setAcking] = useState(false);

  const loadConfig = useCallback(async () => {
    const res = await apiGet<{ config: AlertConfig }>("/api/alerts/config");
    setCfg(normalizeConfig(res.config));
  }, []);

  const loadLogs = useCallback(async () => {
    const r = await apiGet<{ total: number; logs: AlertLog[] }>("/api/alerts/logs?limit=100");
    setLogs(r.logs ?? []);
    setLogsTotal(r.total ?? 0);
    setLogsPage((p) => ({ ...p, page: 1 }));
  }, []);

  useEffect(() => {
    void loadConfig().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [loadConfig, toast]);

  useEffect(() => {
    if (activeTab === "logs") void loadLogs().catch((e) => toast(String(e), false));
  }, [activeTab, loadLogs, toast]);

  const save = async (testAfter = false) => {
    const ok = await nerveConfirm({
      title: testAfter ? "发送测试告警" : "发布告警配置",
      message: testAfter
        ? "确认发送测试告警？\n\n将先发布当前配置，再向已启用的通知通道发送一条测试消息。"
        : "确认发布当前告警配置？\n\n发布后将立即生效，命中安全事件时按此配置推送通知。",
      okText: testAfter ? "发送测试" : "确认发布",
    });
    if (!ok) return;

    setSaving(true);
    setTestResult(testAfter ? "发送中…" : "");
    try {
      await apiPut("/api/alerts/config", cfg);
      if (testAfter) {
        const r = await apiPost<{ ok: boolean; delivered: number; channels: { channel: string; ok: boolean; detail: string }[] }>(
          "/api/alerts/test",
        );
        const names: Record<string, string> = { wecom: "企业微信", email: "邮件", sms: "短信", webhook: "Webhook" };
        const lines = (r.channels ?? []).map((c) => `${names[c.channel] ?? c.channel}: ${c.ok ? "成功" : "失败"} — ${c.detail}`);
        setTestResult(lines.length ? lines.join(" · ") : "未配置任何通知通道");
        toast(r.ok ? `测试告警已发送 (${r.delivered} 个通道)` : "部分或全部通道发送失败", r.ok);
      } else {
        toast("告警配置已发布并生效");
      }
    } catch (e) {
      if (testAfter) setTestResult(String(e));
      toast(String(e), false);
    } finally {
      setSaving(false);
    }
  };

  const runDigest = async (kind: string) => {
    try {
      const r = await apiPost<{ ok: boolean; detail?: string; error?: string }>("/api/alerts/digest/run", { kind });
      toast(r.ok ? (r.detail ?? "已执行") : (r.error ?? "执行失败"), r.ok);
    } catch (e) {
      toast(String(e), false);
    }
  };

  const openAckDialog = (log: AlertLog) => {
    setAckTarget(log);
    setAckReason("");
  };

  const submitAck = async () => {
    if (!ackTarget) return;
    const reason = ackReason.trim();
    if (!reason) {
      toast("请填写确认原因", false);
      return;
    }
    setAcking(true);
    try {
      await apiPost(`/api/alerts/logs/${ackTarget.id}/ack`, {
        acked_by: "console",
        ack_reason: reason,
      });
      toast("已确认告警");
      setAckTarget(null);
      setAckReason("");
      await loadLogs();
    } catch (e) {
      toast(String(e), false);
    } finally {
      setAcking(false);
    }
  };

  const logRows = paginate(logs, logsPage.page, logsPage.pageSize);
  const digest = cfg.digest ?? defaultDigest();

  const panel = useMemo(() => {
    switch (activeTab) {
      case "policy":
        return (
          <NerveForm>
            <NerveFormItem label="最低严重级别">
              <NerveSelect
                className="alert-ctrl-sm"
                value={cfg.min_severity}
                options={SEVERITY_OPTIONS}
                onChange={(e) => setCfg((c) => ({ ...c, min_severity: e.target.value }))}
              />
            </NerveFormItem>
            <NerveFormItem label="告警冷却（秒）">
              <NerveInput
                type="number"
                className="alert-ctrl-sm"
                value={String(cfg.cooldown_sec ?? 300)}
                onChange={(e) => setCfg((c) => ({ ...c, cooldown_sec: Math.max(0, Number(e.target.value) || 0) }))}
              />
              <p className="muted small">同一攻击类型 + 源 IP 在冷却期内只推送一次（0 = 不限制）</p>
            </NerveFormItem>
            <div className="ant-form-section-title">定时摘要 JOB</div>
            <div className="alert-digest-block">
              <NerveSwitch
                label="每日安全摘要"
                checked={digest.daily_enabled}
                onChange={(e) => setCfg((c) => ({ ...c, digest: { ...digest, daily_enabled: e.target.checked } }))}
              />
              <div className="alert-digest-fields">
                <NerveFormItem label="每日摘要时间（UTC 小时）">
                  <NerveInput
                    type="number"
                    className="alert-ctrl-sm"
                    value={String(digest.daily_hour_utc)}
                    onChange={(e) =>
                      setCfg((c) => ({ ...c, digest: { ...digest, daily_hour_utc: Number(e.target.value) } }))
                    }
                  />
                </NerveFormItem>
              </div>
            </div>
            <div className="alert-digest-block">
              <NerveSwitch
                label="未 ACK 告警复查"
                checked={digest.unack_enabled}
                onChange={(e) => setCfg((c) => ({ ...c, digest: { ...digest, unack_enabled: e.target.checked } }))}
              />
            </div>
            <div className="chip-row">
              <NerveButton size="sm" onClick={() => void runDigest("daily")}>立即发送日报</NerveButton>
              <NerveButton size="sm" onClick={() => void runDigest("weekly")}>立即发送周报</NerveButton>
              <NerveButton size="sm" onClick={() => void runDigest("unack")}>立即复查未 ACK</NerveButton>
            </div>
          </NerveForm>
        );
      case "wecom":
        return (
          <NerveForm>
            <NerveSwitch
              label="启用此通道"
              checked={cfg.wecom.enabled}
              onChange={(e) => setCfg((c) => ({ ...c, wecom: { ...c.wecom, enabled: e.target.checked } }))}
            />
            <NerveFormItem label="机器人 Webhook URL">
              <NerveInput
                wide
                type="url"
                value={cfg.wecom.webhook_url}
                onChange={(e) => setCfg((c) => ({ ...c, wecom: { ...c.wecom, webhook_url: e.target.value } }))}
                placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "email":
        return (
          <NerveForm>
            <NerveSwitch
              label="启用此通道"
              checked={cfg.email.enabled}
              onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, enabled: e.target.checked } }))}
            />
            <NerveFormItem label="SMTP 服务器">
              <NerveInput wide value={cfg.email.smtp_host} onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, smtp_host: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="端口">
              <NerveInput
                type="number"
                wide
                value={String(cfg.email.smtp_port)}
                onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, smtp_port: Number(e.target.value) || 465 } }))}
              />
            </NerveFormItem>
            <NerveSwitch
              label="TLS/SSL"
              checked={cfg.email.use_tls}
              onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, use_tls: e.target.checked } }))}
            />
            <NerveFormItem label="用户名">
              <NerveInput wide value={cfg.email.username} onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, username: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="密码 / 授权码">
              <NerveInput wide type="password" value={cfg.email.password} onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, password: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="发件人">
              <NerveInput wide value={cfg.email.from} onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, from: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="收件人（每行一个）">
              <NerveTextarea
                wide
                rows={3}
                value={(cfg.email.to ?? []).join("\n")}
                onChange={(e) => setCfg((c) => ({ ...c, email: { ...c.email, to: splitLines(e.target.value) } }))}
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "sms":
        return (
          <NerveForm>
            <NerveSwitch
              label="启用此通道"
              checked={cfg.sms.enabled}
              onChange={(e) => setCfg((c) => ({ ...c, sms: { ...c.sms, enabled: e.target.checked } }))}
            />
            <NerveFormItem label="网关 Webhook URL">
              <NerveInput wide value={cfg.sms.webhook_url} onChange={(e) => setCfg((c) => ({ ...c, sms: { ...c.sms, webhook_url: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="接收手机号（每行一个）">
              <NerveTextarea
                wide
                rows={3}
                value={(cfg.sms.phones ?? []).join("\n")}
                onChange={(e) => setCfg((c) => ({ ...c, sms: { ...c.sms, phones: splitLines(e.target.value) } }))}
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "webhook":
        return (
          <NerveForm>
            <NerveFormItem label="Webhook URL（每行一个）">
              <NerveTextarea
                wide
                rows={4}
                value={(cfg.webhooks ?? []).join("\n")}
                onChange={(e) => setCfg((c) => ({ ...c, webhooks: splitLines(e.target.value) }))}
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "template":
        return (
          <NerveForm>
            <NerveFormItem label="邮件主题 / 告警标题">
              <NerveInput wide value={cfg.template.title} onChange={(e) => setCfg((c) => ({ ...c, template: { ...c.template, title: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="正文模板">
              <NerveTextarea wide rows={8} value={cfg.template.body} onChange={(e) => setCfg((c) => ({ ...c, template: { ...c.template, body: e.target.value } }))} />
            </NerveFormItem>
            <NerveFormItem label="企业微信 Markdown">
              <NerveTextarea wide rows={8} value={cfg.template.wecom_markdown} onChange={(e) => setCfg((c) => ({ ...c, template: { ...c.template, wecom_markdown: e.target.value } }))} />
            </NerveFormItem>
            <NerveButton size="sm" onClick={() => setCfg((c) => ({ ...c, template: { ...DEFAULT_TEMPLATE } }))}>恢复默认模板</NerveButton>
          </NerveForm>
        );
      case "logs":
        return (
          <>
            <div className="alert-panel-head-row" style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
              <div>
                <h3>告警记录</h3>
                <p className="muted">共 {logsTotal} 条 · 展示最近 {logs.length} 条</p>
              </div>
              <NerveButton size="sm" onClick={() => void loadLogs()}>刷新</NerveButton>
            </div>
            <NerveTable
              columns={[
                { key: "ts", title: "时间", render: (r) => r.ts.replace("T", " ").slice(0, 19) },
                { key: "status", title: "状态", render: (r) => <NerveTag tone={statusTone(r.status)}>{STATUS_LABEL[r.status] ?? r.status}</NerveTag> },
                { key: "trigger", title: "来源", render: (r) => TRIGGER_LABEL[r.trigger] ?? r.trigger },
                { key: "category", title: "类型", render: (r) => categoryLabel(r.category) },
                { key: "src_ip", title: "源 IP", render: (r) => r.src_ip ?? "-" },
                { key: "delivered", title: "通道" },
                {
                  key: "ack",
                  title: "ACK",
                  className: NERVE_COL_ACTIONS,
                  render: (r) =>
                    r.acked ? (
                      <span title={r.ack_reason || undefined}>
                        <NerveTag tone="green">已ACK</NerveTag>
                        {r.ack_reason && (
                          <span className="muted small" style={{ display: "block", marginTop: 2, maxWidth: 160, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {r.ack_reason}
                          </span>
                        )}
                      </span>
                    ) : r.status === "sent" && (r.trigger === "realtime" || r.trigger === "ingest") ? (
                      <NerveTableActions actions={[{ key: "ack", label: "确认", onClick: () => openAckDialog(r) }]} />
                    ) : (
                      "-"
                    ),
                },
              ]}
              dataSource={logRows}
              rowKey="id"
              emptyText="暂无告警记录"
            />
            <NervePagination total={logs.length} page={logsPage.page} pageSize={logsPage.pageSize} onChange={setLogsPage} />
          </>
        );
      default:
        return null;
    }
  }, [activeTab, cfg, digest, logRows, logs.length, logsTotal, logsPage]);

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      <NervePageHeader title="告警配置" />
      <NerveCard title="">
        <div className="alert-console-toolbar">
          <NerveSwitch
            label="启用告警"
            checked={cfg.enabled}
            onChange={(e) => setCfg((c) => ({ ...c, enabled: e.target.checked }))}
          />
          <span className="alert-toolbar-spacer" />
          <NerveButton disabled={saving} onClick={() => void save(true)}>发送测试</NerveButton>
          <NerveButton variant="primary" disabled={saving} onClick={() => void save(false)}>发布</NerveButton>
        </div>
        <div className="alert-console-body">
          <nav className="alert-console-nav">
            {NAV_ITEMS.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`alert-nav-item${activeTab === item.id ? " active" : ""}`}
                onClick={() => setActiveTab(item.id)}
              >
                <span className="alert-nav-icon">{item.icon}</span>
                <span className="alert-nav-text">
                  <strong>{item.label}</strong>
                  <span className="muted small">{item.desc}</span>
                </span>
                {item.id !== "policy" && item.id !== "template" && item.id !== "logs" ? (
                  <span className={`alert-nav-dot ${channelReady(cfg, item.id) ? "on" : ""}`} />
                ) : null}
              </button>
            ))}
          </nav>
          <div className="alert-console-panel">{panel}</div>
        </div>
        {testResult ? <div className="alert-console-footer muted small">{testResult}</div> : null}
      </NerveCard>

      {ackTarget && (
        <NerveModal
          open
          title="确认告警"
          onClose={() => setAckTarget(null)}
          footer={
            <>
              <NerveButton onClick={() => setAckTarget(null)}>取消</NerveButton>
              <NerveButton variant="primary" disabled={acking} onClick={() => void submitAck()}>
                {acking ? "提交中…" : "确认"}
              </NerveButton>
            </>
          }
        >
          <div style={{ minWidth: 420 }}>
            <NerveFormItem label="告警信息">
              <div className="muted small">
                {ackTarget.category} · {ackTarget.src_ip ?? "-"} · {ackTarget.ts.replace("T", " ").slice(0, 19)}
              </div>
            </NerveFormItem>
            <NerveFormItem label="确认原因 / 处理备注" required>
              <NerveTextarea
                wide
                rows={4}
                value={ackReason}
                onChange={(e) => setAckReason(e.target.value)}
                placeholder="请填写确认原因或处理备注，例如：已确认为误报，已通知业务方处理。"
              />
            </NerveFormItem>
          </div>
        </NerveModal>
      )}
    </>
  );
}
