import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveModal,
  NerveTabBar,
  NerveTable,
  NerveTag,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiGet, apiPost, apiPut } from "../../api";
import { sanitizeProductText } from "../../brand-sanitize";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "../../crs-preset";
import { SecurityGraphPanel } from "./SecurityGraphPanel";
import { categoryLabel } from "../../lib/labels";
import { ruleHintById } from "../../lib/crs-rules";
import {
  resolveModuleKeyForEvent,
  triageTone,
  triageVerdictLabel,
} from "../../lib/events-display";
import type { ProtectionModules, TriageSummary, WafEvent } from "../../lib/waf-types";
import { MODULE_META } from "../../module-meta";
import { siteDisplayName, type ProtectedSite } from "../../sites";
import { TempBanDialog } from "../access/TempBanDialog";
import { ModuleRulesModal } from "../modules/ModuleRulesModal";

type WhitelistConfig = {
  enabled: boolean;
  rules: {
    id: string;
    enabled: boolean;
    comment: string;
    match_on: {
      hosts?: string[];
      uri_prefixes?: string[];
      rule_ids?: string[];
      categories?: string[];
      src_ips?: string[];
    };
  }[];
};

type Props = {
  open: boolean;
  event: WafEvent | null;
  sites: ProtectedSite[];
  onClose: () => void;
};

type DetailTab = "triage" | "graph";

export function TriageDetailModal({ open, event, sites, onClose }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [triage, setTriage] = useState<TriageSummary | null>(null);
  const [detailTab, setDetailTab] = useState<DetailTab>("triage");
  const [rulesOpen, setRulesOpen] = useState(false);
  const [banOpen, setBanOpen] = useState(false);
  const [rulesCtx, setRulesCtx] = useState<{
    category: string;
    protect: 1 | 2 | 3;
    title: string;
    key: string;
    siteId: string;
    disabled: string[];
    highlight?: string;
  } | null>(null);

  useEffect(() => {
    if (!open || !event) {
      setTriage(null);
      setError(null);
      setDetailTab("triage");
      return;
    }
    let cancelled = false;
    void (async () => {
      setLoading(true);
      setError(null);
      try {
        let t = event.triage ?? null;
        if (!t?.evidence) {
          const res = await apiPost<{ triage: TriageSummary }>("/api/triage/analyze", { event });
          t = res.triage;
        }
        if (!cancelled) setTriage(t);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, event]);

  const confirmFalsePositive = useCallback(async () => {
    if (!event) return;
    const siteId = (event.site_id ?? "default").trim() || "default";
    const uri = event.uri ?? "—";
    const ok = await nerveConfirm({
      title: "上报误报",
      message: "确认将本条事件标记为误报？提交后将计入误报统计，用于后续策略优化。",
      okText: "确认上报",
    });
    if (!ok) return;
    try {
      await apiPost<{ ok: boolean; applied: string[]; audit_id: string }>(
        `/api/events/${encodeURIComponent(event.id)}/confirm-false-positive`,
        { site_id: siteId, actions: [], note: `Console 上报误报 ${uri}` },
      );
      await nerveResult({
        type: "success",
        title: "提交成功",
        message: "误报已记录。",
      });
      onClose();
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "提交失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, [event, onClose]);

  const whitelistIp = useCallback(async () => {
    if (!event) return;
    const ip = (event.src_ip ?? "").trim();
    if (!ip) {
      await nerveResult({ type: "error", title: "无法加白", message: "事件缺少源 IP" });
      return;
    }
    const ok = await nerveConfirm({
      title: `加白源 IP ${ip}`,
      message: `将 ${ip} 加入全局白名单（按源 IP 放行）。确认继续？`,
      okText: "确认加白",
    });
    if (!ok) return;
    try {
      const res = await apiGet<{ config: WhitelistConfig }>("/api/whitelist");
      const cfg: WhitelistConfig = res.config ?? { enabled: true, rules: [] };
      cfg.enabled = true;
      const exists = (cfg.rules ?? []).some(
        (r) => r.enabled && (r.match_on?.src_ips ?? []).includes(ip),
      );
      if (!exists) {
        cfg.rules = [
          ...(cfg.rules ?? []),
          {
            id: `ev-wl-${Date.now().toString(36)}`,
            enabled: true,
            comment: `攻击日志加白 · ${event.id}`,
            match_on: { src_ips: [ip] },
          },
        ];
      }
      await apiPut("/api/whitelist", cfg);
      await nerveResult({ type: "success", title: "已加白", message: `${ip} 已加入白名单` });
    } catch (e) {
      await nerveResult({ type: "error", title: "加白失败", message: String(e) });
    }
  }, [event]);

  const openRuleMgr = useCallback(async () => {
    if (!event) return;
    const siteId = (event.site_id ?? "default").trim() || "default";
    const moduleKey = resolveModuleKeyForEvent(event);
    let category = MODULE_CRS_CATEGORY[moduleKey] ?? (moduleKey !== "custom" ? moduleKey : null);
    if (!category && event.category !== "custom" && event.category !== "l3_policy") {
      category = MODULE_CRS_CATEGORY[event.category] ?? event.category;
    }
    if (!category || category === "custom" || event.category === "custom") {
      await nerveResult({
        type: "info",
        title: "自定义规则",
        message: "此事件由自定义规则命中。",
      });
      return;
    }
    try {
      const modules = await apiGet<ProtectionModules>(
        `/api/modules?site_id=${encodeURIComponent(siteId)}`,
      );
      const cfg = modules.modules[moduleKey] ?? modules.modules[category] ?? {
        enabled: true,
        action: "block",
        sensitivity: "medium",
      };
      const modMeta =
        MODULE_META[moduleKey] ?? MODULE_META[category] ?? { title: categoryLabel(category), desc: "" };
      const saveKey = modules.modules[moduleKey] ? moduleKey : category;
      setRulesCtx({
        category,
        protect: sensitivityToProtectLevel(cfg.sensitivity),
        title: modMeta.title,
        key: saveKey,
        siteId,
        disabled: cfg.disabled_rules ?? [],
        highlight: event.rule_id,
      });
      setRulesOpen(true);
    } catch (e) {
      await nerveResult({ type: "error", title: "加载失败", message: String(e) });
    }
  }, [event]);

  const hasIp = !!(event?.src_ip ?? "").trim();
  const hint =
    event?.rule_id != null
      ? ruleHintById(event.rule_id, event.rule_name, event.category)
      : null;

  return (
    <>
      <NerveModal
        open={open && !!event}
        xl
        title="智能研判详情"
        onClose={onClose}
        footer={
          <>
            {hasIp ? (
              <>
                <NerveButton variant="danger" onClick={() => setBanOpen(true)}>
                  封禁源 IP
                </NerveButton>
                <NerveButton onClick={() => void whitelistIp()}>加白源 IP</NerveButton>
              </>
            ) : null}
            <NerveButton onClick={() => void confirmFalsePositive()}>上报误报</NerveButton>
          </>
        }
      >
        {loading ? <div className="page-loading">加载研判信息…</div> : null}
        {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
        {!loading && !error && event ? (
          <>
            <NerveTabBar
              tabs={[
                { id: "triage", label: "研判" },
                { id: "graph", label: "安全图谱" },
              ]}
              activeId={detailTab}
              onChange={(id) => setDetailTab(id as DetailTab)}
            />
            {detailTab === "graph" ? (
              <div style={{ marginTop: 16 }}>
                <SecurityGraphPanel
                  compact
                  eventId={event.id}
                  srcIp={event.src_ip ?? ""}
                />
              </div>
            ) : null}
            {detailTab === "triage" && triage ? (
          <div style={{ marginTop: 16 }}>
            <div style={{ marginBottom: 12, display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <NerveTag tone={triageTone(triage.verdict)}>{triageVerdictLabel(triage.verdict)}</NerveTag>
              <span className="muted small">{triage.confidence}%</span>
              <span className="muted">
                风险分 {triage.risk_score} · {triage.engine}
              </span>
            </div>
            <p>{triage.summary}</p>

            <div className="triage-rule-block">
              <div className="triage-rule-head">
                <strong>命中规则</strong>
                <NerveButton size="sm" variant="primary" onClick={() => void openRuleMgr()}>
                  打开应用规则配置
                </NerveButton>
              </div>
              {!event.rule_id?.trim() && !event.rule_name?.trim() ? (
                <div className="muted">未关联具体规则 ID（语义/行为类检测）</div>
              ) : (
                <table className="data-table triage-rule-table">
                  <tbody>
                    <tr>
                      <td className="muted small">规则 ID</td>
                      <td>
                        <code>{event.rule_id ?? "—"}</code>
                      </td>
                    </tr>
                    <tr>
                      <td className="muted small">名称</td>
                      <td>{sanitizeProductText(event.rule_name ?? "—")}</td>
                    </tr>
                    <tr>
                      <td className="muted small">防护来源</td>
                      <td>
                        {[event.strategy_name, event.protection_stage, event.rule_set, event.plugin_name]
                          .filter(Boolean)
                          .join(" · ") || "—"}
                      </td>
                    </tr>
                    <tr>
                      <td className="muted small">类型</td>
                      <td>{categoryLabel(event.category)}</td>
                    </tr>
                    <tr>
                      <td className="muted small">严重度</td>
                      <td>{event.severity}</td>
                    </tr>
                    {hint ? (
                      <tr>
                        <td className="muted small">检测说明</td>
                        <td>
                          {hint.desc}
                          {hint.example ? (
                            <>
                              <br />
                              <code className="small rule-hint-ex">{hint.example}</code>
                            </>
                          ) : null}
                        </td>
                      </tr>
                    ) : null}
                  </tbody>
                </table>
              )}
            </div>

            {triage.ai_summary ? (
              <div
                className="triage-ai-block"
                style={{
                  marginTop: 12,
                  padding: 12,
                  background: "var(--bg-soft,#f6ffed)",
                  border: "1px solid var(--border,#b7eb8f)",
                  borderRadius: 6,
                }}
              >
                <strong>AI 解释</strong>
                <span className="muted small">
                  {" "}
                  · {triage.ai_engine ?? "ai"}
                  {triage.ai_model ? ` · ${triage.ai_model}` : ""}
                </span>
                <div style={{ marginTop: 8, whiteSpace: "pre-wrap" }}>{triage.ai_summary}</div>
              </div>
            ) : null}

            <h4 className="small">证据链</h4>
            <NerveTable
              columns={[
                { key: "signal", title: "信号", render: (r) => <code>{r.signal}</code> },
                {
                  key: "weight",
                  title: "权重",
                  render: (r) => `${r.weight > 0 ? "+" : ""}${r.weight}`,
                },
                { key: "detail", title: "说明" },
              ]}
              dataSource={triage.evidence ?? []}
              rowKey={(_, i) => String(i)}
              emptyText="—"
            />

            <h4 className="small" style={{ marginTop: 12 }}>
              建议
            </h4>
            <ul>
              {(triage.recommendations ?? []).length ? (
                (triage.recommendations ?? []).map((r, i) => <li key={i}>{r}</li>)
              ) : (
                <li className="muted">—</li>
              )}
            </ul>

            <p className="muted small">
              事件 ID: <code>{event.id}</code>
              {event.site_id ? (
                <>
                  {" "}
                  · 应用: {siteDisplayName(sites, event.site_id)} <code>{event.site_id}</code>
                </>
              ) : null}
            </p>
          </div>
            ) : detailTab === "triage" && !triage ? (
              <p className="muted small" style={{ marginTop: 16 }}>暂无研判数据</p>
            ) : null}
          </>
        ) : null}
      </NerveModal>

      <TempBanDialog
        open={banOpen}
        ip={(event?.src_ip ?? "").trim()}
        source="console-events"
        defaultReason={`攻击日志 · ${event?.id ?? ""}`}
        onClose={() => setBanOpen(false)}
      />

      {rulesCtx ? (
        <ModuleRulesModal
          open={rulesOpen}
          onClose={() => setRulesOpen(false)}
          category={rulesCtx.category}
          protect={rulesCtx.protect}
          moduleTitle={rulesCtx.title}
          moduleKey={rulesCtx.key}
          siteId={rulesCtx.siteId}
          editable
          disabledRules={rulesCtx.disabled}
          highlightRuleId={rulesCtx.highlight}
          onSave={(disabledRules) => {
            void (async () => {
              try {
                const modules = await apiGet<ProtectionModules>(
                  `/api/modules?site_id=${encodeURIComponent(rulesCtx.siteId)}`,
                );
                const next: ProtectionModules = {
                  ...modules,
                  modules: { ...modules.modules },
                };
                const base = next.modules[rulesCtx.key] ?? {
                  enabled: true,
                  action: "block",
                  sensitivity: "medium",
                };
                next.modules[rulesCtx.key] = {
                  ...base,
                  disabled_rules: disabledRules.length ? disabledRules : undefined,
                };
                await apiPut(`/api/modules?site_id=${encodeURIComponent(rulesCtx.siteId)}`, next);
                await nerveResult({
                  type: "success",
                  title: "规则已保存",
                  message: `应用「${siteDisplayName(sites, rulesCtx.siteId)}」的规则开关已更新`,
                });
              } catch (err) {
                await nerveResult({ type: "error", title: "保存失败", message: String(err) });
              }
            })();
          }}
        />
      ) : null}
    </>
  );
}
