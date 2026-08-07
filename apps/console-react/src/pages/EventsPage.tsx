import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NervePageHeader,
  NervePagination,
  NerveSelect,
  NerveTable,
  NerveTableToolbar,
  NerveTag,
  defaultPaginationState,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiGet, apiPost, apiPut } from "../api";
import { sanitizeProductText } from "../brand-sanitize";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "../crs-preset";
import { TempBanDialog } from "../features/access/TempBanDialog";
import { RawRequestModal } from "../features/events/RawRequestModal";
import { TriageDetailModal } from "../features/events/TriageDetailModal";
import { ModuleRulesModal } from "../features/modules/ModuleRulesModal";
import { categoryLabel } from "../lib/labels";
import {
  confidenceClass,
  eventRequestMessage,
  eventSample,
  moduleDisplay,
  resolveModuleKeyForEvent,
  strategyDisplay,
  triageTone,
  triageVerdictClass,
  triageVerdictLabel,
} from "../lib/events-display";
import { buildEventsListQuery } from "../lib/events-list-query";
import type { ProtectionModules, WafEvent } from "../lib/waf-types";
import { MODULE_META } from "../module-meta";
import { PAGE_META } from "../nav";
import { consumeEventsSiteFilter, loadSites, siteDisplayName, type ProtectedSite } from "../sites";
import { ActionTag, Ellipsis, formatTsEvent } from "./shared";

const EVENT_CATEGORY_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "sqli", label: "SQL 注入" },
  { value: "xss", label: "XSS" },
  { value: "rce", label: "RCE" },
  { value: "custom", label: "自定义规则" },
];

type WhitelistConfig = {
  enabled: boolean;
  rules: {
    id: string;
    enabled: boolean;
    comment: string;
    match_on: { src_ips?: string[] };
  }[];
};

export function EventsPage() {
  const meta = PAGE_META.events;
  const [sites, setSites] = useState<ProtectedSite[]>([]);
  const [siteFilter, setSiteFilter] = useState("");
  const [catFilter, setCatFilter] = useState("");
  const [events, setEvents] = useState<WafEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pagination, setPagination] = useState(defaultPaginationState());
  const [triageEvent, setTriageEvent] = useState<WafEvent | null>(null);
  const [rawEvent, setRawEvent] = useState<WafEvent | null>(null);
  const [banIp, setBanIp] = useState<string | null>(null);
  const [rulesOpen, setRulesOpen] = useState(false);
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
    void loadSites()
      .then(setSites)
      .catch(() => setSites([{ id: "default", name: "系统默认", hosts: ["*"], enabled: true }]));
    const initial = consumeEventsSiteFilter();
    if (initial) setSiteFilter(initial);
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const q = buildEventsListQuery({
        page: pagination.page,
        pageSize: pagination.pageSize,
        category: catFilter,
        siteId: siteFilter,
      });
      const data = await apiGet<{ total: number; events: WafEvent[] }>(
        `/api/events?${q}`,
      );
      setEvents(data.events);
      setTotal(data.total);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [catFilter, siteFilter, pagination.page, pagination.pageSize]);

  useEffect(() => {
    void load();
  }, [load]);

  const siteOptions = useMemo(
    () => [
      { value: "", label: "全部应用" },
      ...sites.map((s) => ({
        value: s.id,
        label: `${siteDisplayName(sites, s.id)} (${s.id})`,
      })),
    ],
    [sites],
  );

  const openEventRuleManager = useCallback(
    async (event: WafEvent) => {
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
    },
    [],
  );

  const confirmFalsePositive = useCallback(async (event: WafEvent) => {
    const siteId = (event.site_id ?? "default").trim() || "default";
    const uri = event.uri ?? "—";
    const ok = await nerveConfirm({
      title: "上报误报",
      message: "确认将本条事件标记为误报？提交后将计入误报统计，用于后续策略优化。",
      okText: "确认上报",
    });
    if (!ok) return;
    try {
      await apiPost<{ ok: boolean; applied: string[] }>(
        `/api/events/${encodeURIComponent(event.id)}/confirm-false-positive`,
        { site_id: siteId, actions: [], note: `Console 上报误报 ${uri}` },
      );
      await nerveResult({
        type: "success",
        title: "提交成功",
        message: "误报已记录。",
      });
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "提交失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, []);

  const whitelistEventIp = useCallback(async (event: WafEvent) => {
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
  }, []);

  return (
    <>
      <NervePageHeader title={meta.title} subtitle={meta.subtitle} />
      <NerveTableToolbar>
        <label className="muted small">
          应用{" "}
          <NerveSelect
            value={siteFilter}
            options={siteOptions}
            style={{ width: 200, marginLeft: 8 }}
            onChange={(e) => {
              setSiteFilter(e.target.value);
              setPagination((p) => ({ ...p, page: 1 }));
            }}
          />
        </label>
        <label className="muted small">
          类型{" "}
          <NerveSelect
            value={catFilter}
            options={EVENT_CATEGORY_OPTIONS}
            style={{ width: 160, marginLeft: 8 }}
            onChange={(e) => {
              setCatFilter(e.target.value);
              setPagination((p) => ({ ...p, page: 1 }));
            }}
          />
        </label>
        <NerveButton variant="primary" onClick={() => void load()}>
          刷新
        </NerveButton>
      </NerveTableToolbar>

      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}

      <NerveCard title="L7 攻击事件">
        {loading ? <div className="page-loading">加载中…</div> : null}
        {!loading && events.length === 0 ? (
          <div className="muted">暂无事件</div>
        ) : null}
        {!loading && events.length > 0 ? (
          <>
            <div className="muted table-summary small" style={{ marginBottom: 8 }}>
              共 {total} 条
            </div>
            <div className="events-l7-table-wrap">
              <NerveTable<WafEvent>
                className="events-l7-table"
                scroll={{ x: "max-content" }}
                rowKey="id"
                dataSource={events}
                columns={[
                  {
                    key: "ts",
                    title: "时间",
                    className: "ev-col-full",
                    nowrap: true,
                    render: (e) => <code className="ev-ts">{formatTsEvent(e.ts)}</code>,
                  },
                  {
                    key: "site",
                    title: "应用",
                    className: "ev-col-full",
                    nowrap: true,
                    render: (e) => (
                      <span
                        className="app-event-tag ev-app-tag"
                        title={`${siteDisplayName(sites, e.site_id)} (${e.site_id ?? "default"})`}
                      >
                        {siteDisplayName(sites, e.site_id)}
                      </span>
                    ),
                  },
                  {
                    key: "action",
                    title: "动作",
                    nowrap: true,
                    render: (e) => <ActionTag action={e.action} />,
                  },
                  {
                    key: "strategy",
                    title: "策略",
                    width: 88,
                    render: (e) => {
                      const s = strategyDisplay(e);
                      if (!s) return <span className="muted">—</span>;
                      return <Ellipsis text={s.name} />;
                    },
                  },
                  {
                    key: "module",
                    title: "模块",
                    className: "ev-col-full",
                    nowrap: true,
                    render: (e) => {
                      const m = moduleDisplay(e);
                      if (!m) return <span className="muted">—</span>;
                      const label = [m.stage, m.ruleSet].filter(Boolean).join(" · ");
                      return <span>{label}</span>;
                    },
                  },
                  {
                    key: "rule",
                    title: "命中规则",
                    width: 140,
                    render: (e) => {
                      const name = sanitizeProductText(e.rule_name?.trim() ?? "");
                      const id = e.rule_id?.trim();
                      if (!name && !id) return <span className="muted">—</span>;
                      const label = name || id || "—";
                      const showIdJump = !!(id && ((name && id !== name) || !name));
                      return (
                        <div className="ev-rule-cell ev-rule-compact">
                          <Ellipsis text={label} />
                          {showIdJump ? (
                            <button
                              type="button"
                              className="ant-btn-link ant-btn-sm ev-rule-jump"
                              title={`查看规则 ${id}`}
                              onClick={() => void openEventRuleManager(e)}
                            >
                              <code>{id}</code>
                            </button>
                          ) : null}
                        </div>
                      );
                    },
                  },
                  {
                    key: "ip",
                    title: "IP",
                    className: "ev-col-full",
                    nowrap: true,
                    render: (e) => <code className="ev-ip">{e.src_ip ?? e.dst_ip ?? "—"}</code>,
                  },
                  {
                    key: "req",
                    title: "请求",
                    width: 280,
                    render: (e) => {
                      const msg = eventRequestMessage(e);
                      return (
                        <code className="ev-req-line">
                          <Ellipsis text={msg} />
                          {e.message?.includes("semantic_score=") ? (
                            <span className="muted ev-semantic-hint" title={e.message ?? ""}>
                              {" "}
                              semantic
                            </span>
                          ) : null}
                        </code>
                      );
                    },
                  },
                  {
                    key: "raw_req",
                    title: "原始请求",
                    width: 96,
                    className: "ev-col-full",
                    nowrap: true,
                    render: (e) => (
                      <button
                        type="button"
                        className="ant-btn-link ant-btn-sm ev-raw-req-view"
                        onClick={() => setRawEvent(e)}
                      >
                        查看
                      </button>
                    ),
                  },
                  {
                    key: "sample",
                    title: "样本",
                    width: 120,
                    render: (e) => <Ellipsis text={eventSample(e) || "—"} className="muted ev-ellipsis" />,
                  },
                  {
                    key: "triage",
                    title: "研判",
                    fixed: "right",
                    width: 88,
                    className: "ev-col-triage nerve-col-fixed-right",
                    nowrap: true,
                    render: (e) =>
                      e.triage ? (
                        <span className={triageVerdictClass(e.triage.verdict)}>
                          <NerveTag tone={triageTone(e.triage.verdict)}>
                            {triageVerdictLabel(e.triage.verdict)}
                          </NerveTag>
                        </span>
                      ) : (
                        <span className="muted">—</span>
                      ),
                  },
                  {
                    key: "confidence",
                    title: "置信",
                    fixed: "right",
                    width: 96,
                    className: "ev-col-full ev-col-confidence nerve-col-fixed-right",
                    nowrap: true,
                    render: (e) =>
                      e.triage ? (
                        <span className="ev-confidence-cell">
                          <span
                            className={confidenceClass(e.triage.confidence)}
                            title={`研判置信 ${e.triage.confidence}%`}
                          >
                            {e.triage.confidence}%
                          </span>
                          <button
                            type="button"
                            className="ant-btn-link ant-btn-sm ev-triage-view"
                            onClick={() => setTriageEvent(e)}
                          >
                            详情
                          </button>
                        </span>
                      ) : (
                        <span className="muted">—</span>
                      ),
                  },
                  {
                    key: "dispose",
                    title: "操作",
                    fixed: "right",
                    width: 132,
                    className: "nerve-col-actions",
                    nowrap: true,
                    render: (e) => {
                      const hasIp = !!(e.src_ip ?? "").trim();
                      return (
                        <span className="ev-actions-cell">
                          {hasIp ? (
                            <button
                              type="button"
                              className="ant-btn-link ant-btn-sm ev-temp-ban"
                              onClick={() => setBanIp((e.src_ip ?? "").trim())}
                            >
                              封禁
                            </button>
                          ) : (
                            <span className="muted" title="事件缺少源 IP">
                              封禁
                            </span>
                          )}
                          {hasIp ? (
                            <button
                              type="button"
                              className="ant-btn-link ant-btn-sm ev-wl-ip"
                              onClick={() => void whitelistEventIp(e)}
                            >
                              加白
                            </button>
                          ) : (
                            <span className="muted" title="事件缺少源 IP">
                              加白
                            </span>
                          )}
                          <button
                            type="button"
                            className="ant-btn-link ant-btn-sm ev-fp-confirm"
                            onClick={() => void confirmFalsePositive(e)}
                          >
                            误报
                          </button>
                        </span>
                      );
                    },
                  },
                ]}
              />
            </div>
            <NervePagination
              total={total}
              page={pagination.page}
              pageSize={pagination.pageSize}
              onChange={setPagination}
            />
          </>
        ) : null}
      </NerveCard>

      <RawRequestModal
        open={!!rawEvent}
        event={rawEvent}
        onClose={() => setRawEvent(null)}
      />

      <TriageDetailModal
        open={!!triageEvent}
        event={triageEvent}
        sites={sites}
        onClose={() => setTriageEvent(null)}
      />

      <TempBanDialog
        open={!!banIp}
        ip={banIp ?? ""}
        source="console-events"
        defaultReason="攻击日志处置"
        onClose={() => setBanIp(null)}
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
