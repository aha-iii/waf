/** Legacy console module: Security view. */
export type WafEvent = {
  id: string;
  ts: string;
  site_id?: string;
  layer: string;
  action: string;
  category: string;
  rule_id?: string;
  rule_name?: string;
  severity: string;
  src_ip?: string;
  dst_ip?: string;
  uri?: string;
  method?: string;
  host?: string;
  payload_sample?: string;
  message?: string;
  pod?: string;
  inspect_source?: string;
  strategy_id?: string;
  strategy_name?: string;
  protection_stage?: string;
  rule_set?: string;
  plugin_name?: string;
  triage?: TriageSummary;
};

export type TriageSummary = {
  verdict: string;
  confidence: number;
  risk_score: number;
  summary: string;
  engine: string;
  evidence?: { signal: string; weight: number; detail: string }[];
  recommendations?: string[];
  ai_summary?: string;
  ai_engine?: string;
  ai_model?: string;
};

export type ModuleConfig = {
  enabled: boolean;
  action: string;
  sensitivity: string;
  disabled_rules?: string[];
};

export type ProtectionModules = {
  version: number;
  mode: string;
  modules: Record<string, ModuleConfig>;
};

export type StatsOverview = {
  total_events: number;
  blocked: number;
  logged: number;
  by_category: { category: string; count: number }[];
  top_ips: { src_ip: string; count: number }[];
};

import { apiGet, apiPost, apiPut, apiTempDeny } from "./api.js";
import { GLOBAL_DEFAULT_SITE_ID, loadSites, siteDisplayName, siteQuery, consumeEventsSiteFilter, type ProtectedSite } from "./sites.js";
import { bindModuleCardStyles } from "./module-display.js";
import { MODULE_META, orderedWebCoreModuleEntries } from "./module-meta.js";
import {
  actionTag,
  bindPaginationBar,
  btn,
  card,
  categoryLabel,
  defaultPaginationState,
  esc,
  filterSelect,
  hideModal,
  pageHeader,
  renderPaginatedSection,
  runMutatingAction,
  showConfirm,
  showResult,
  tableToolbar,
  type PaginationState,
} from "./components/ui.js";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "./crs-preset.js";
import { ruleHintById } from "./crs-rules-shared.js";
import { sanitizeProductText } from "./brand-sanitize.js";
import { formatTsEvent } from "../lib/datetime.js";
import { showModuleRulesModal } from "./module-rules-modal.js";
import { PAGE_META } from "./nav.js";

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

function el(tag: string, cls?: string, html?: string): HTMLElement {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (html != null) n.innerHTML = html;
  return n;
}

const EVENT_CATEGORY_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "sqli", label: "SQL 注入" },
  { value: "xss", label: "XSS" },
  { value: "rce", label: "RCE" },
  { value: "custom", label: "自定义规则" },
];

export async function renderSecurityEvents(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const meta = PAGE_META.events;
  root.innerHTML = `
    ${pageHeader(meta.title, meta.subtitle)}
    ${tableToolbar(
      filterSelect({ id: "ev-filter-site", label: "应用", width: 200, options: [{ value: "", label: "全部应用" }] }),
      filterSelect({ id: "ev-filter-cat", label: "类型", width: 160, options: EVENT_CATEGORY_OPTIONS }),
      btn({ id: "btn-refresh-ev", label: "刷新", variant: "primary" }),
    )}
    ${card({ id: "ev-table-card", title: "L7 攻击事件", bodyId: "ev-table", body: '<div class="page-loading">加载中…</div>' })}`;

  let cachedEvents: WafEvent[] = [];

  function triageVerdictLabel(verdict: string): string {
    const map: Record<string, string> = {
      confirmed_attack: "确认攻击",
      likely_attack: "疑似攻击",
      needs_review: "待复核",
      likely_false_positive: "疑似误报",
    };
    return map[verdict] ?? verdict;
  }

  function triageVerdictClass(verdict: string): string {
    const map: Record<string, string> = {
      confirmed_attack: "ev-triage-confirmed",
      likely_attack: "ev-triage-likely",
      needs_review: "ev-triage-review",
      likely_false_positive: "ev-triage-fp",
    };
    return map[verdict] ?? "";
  }

  function triageBadge(verdict: string, confidence: number): string {
    const map: Record<string, { label: string; cls: string }> = {
      confirmed_attack: { label: "确认攻击", cls: "ant-tag-error" },
      likely_attack: { label: "疑似攻击", cls: "ant-tag-warning" },
      needs_review: { label: "待复核", cls: "ant-tag-default" },
      likely_false_positive: { label: "疑似误报", cls: "ant-tag-success" },
    };
    const m = map[verdict] ?? { label: verdict, cls: "ant-tag-default" };
    return `<span class="ant-tag ${m.cls}">${m.label}</span> <span class="muted small">${confidence}%</span>`;
  }

  function resolveModuleKeyForEvent(event: WafEvent): string {
    const cat = event.category.trim();
    if (cat && cat !== "custom" && cat !== "l3_policy") return cat;
    const rid = event.rule_id ?? "";
    if (/^\d/.test(rid)) return "crs";
    return "custom";
  }

  function renderEventRuleBlock(event: WafEvent): string {
    const rid = event.rule_id?.trim();
    const rname = event.rule_name?.trim();
    if (!rid && !rname) {
      return `<div class="triage-rule-block muted">未关联具体规则 ID（语义/行为类检测）</div>`;
    }
    const hint = rid ? ruleHintById(rid, rname, event.category) : null;
    return `
      <div class="triage-rule-block">
        <div class="triage-rule-head">
          <strong>命中规则</strong>
          <button type="button" class="ant-btn ant-btn-primary ant-btn-sm ev-open-rule-mgr" data-ev-id="${esc(event.id)}">打开应用规则配置</button>
        </div>
        <table class="data-table triage-rule-table">
          <tbody>
            <tr><td class="muted small">规则 ID</td><td><code>${esc(rid ?? "—")}</code></td></tr>
            <tr><td class="muted small">名称</td><td>${esc(rname ?? "—")}</td></tr>
            <tr><td class="muted small">防护来源</td><td>${esc(
              [
                event.strategy_name,
                event.protection_stage,
                event.rule_set,
                event.plugin_name,
              ]
                .filter(Boolean)
                .join(" · ") || "—",
            )}</td></tr>
            <tr><td class="muted small">类型</td><td>${esc(categoryLabel(event.category))}</td></tr>
            <tr><td class="muted small">严重度</td><td>${esc(event.severity)}</td></tr>
            ${
              hint
                ? `<tr><td class="muted small">检测说明</td><td>${esc(hint.desc)}${
                    hint.example ? `<br><code class="small rule-hint-ex">${esc(hint.example)}</code>` : ""
                  }</td></tr>`
                : ""
            }
          </tbody>
        </table>
      </div>`;
  }

  function renderRuleCell(e: WafEvent): string {
    const name = sanitizeProductText(e.rule_name?.trim() ?? "");
    const id = e.rule_id?.trim();
    if (!name && !id) return `<span class="muted">—</span>`;
    const label = name || id || "—";
    const idLine = id && name && id !== name ? `<br><button type="button" class="ant-btn-link ant-btn-sm ev-rule-jump" data-ev-id="${esc(e.id)}" title="查看规则并配置"><code>${esc(id)}</code></button>` : id && !name ? `<button type="button" class="ant-btn-link ant-btn-sm ev-rule-jump" data-ev-id="${esc(e.id)}"><code>${esc(id)}</code></button>` : "";
    return `<div class="ev-rule-cell"><span>${esc(label)}</span>${idLine}</div>`;
  }

  async function openEventRuleManager(event: WafEvent, onClose?: () => void): Promise<void> {
    const siteId = (event.site_id ?? "default").trim() || "default";
    const moduleKey = resolveModuleKeyForEvent(event);
    let category = MODULE_CRS_CATEGORY[moduleKey] ?? (moduleKey !== "custom" ? moduleKey : null);
    if (!category && event.category !== "custom" && event.category !== "l3_policy") {
      category = MODULE_CRS_CATEGORY[event.category] ?? event.category;
    }
    if (!category || category === "custom" || event.category === "custom") {
      await showResult({
        type: "info",
        title: "自定义规则",
        message: "此事件由自定义规则命中。",
      });
      return;
    }

    onClose?.();
    hideModal();

    let modules: ProtectionModules;
    try {
      modules = await apiGet<ProtectionModules>(`/api/modules?site_id=${encodeURIComponent(siteId)}`);
    } catch (e) {
      await showResult({ type: "error", title: "加载失败", message: String(e) });
      return;
    }

    const cfg = modules.modules[moduleKey] ?? modules.modules[category] ?? {
      enabled: true,
      action: "block",
      sensitivity: "medium",
    };
    const modMeta = MODULE_META[moduleKey] ?? MODULE_META[category] ?? { title: categoryLabel(category), desc: "" };
    const saveKey = modules.modules[moduleKey] ? moduleKey : category;

    await showModuleRulesModal({
      category,
      protect: sensitivityToProtectLevel(cfg.sensitivity),
      moduleTitle: modMeta.title,
      moduleEnabled: cfg.enabled !== false,
      moduleKey: saveKey,
      siteId,
      editable: true,
      disabledRules: cfg.disabled_rules ?? [],
      highlightRuleId: event.rule_id,
      onSave: (disabledRules) => {
        void (async () => {
          const next: ProtectionModules = { ...modules, modules: { ...modules.modules } };
          next.modules[saveKey] = {
            ...(next.modules[saveKey] ?? cfg),
            disabled_rules: disabledRules.length ? disabledRules : undefined,
          };
          try {
            await apiPut(`/api/modules?site_id=${encodeURIComponent(siteId)}`, next);
            await showResult({
              type: "success",
              title: "规则已保存",
              message: `应用「${siteDisplayName(siteList, siteId)}」的规则开关已更新`,
            });
          } catch (err) {
            await showResult({ type: "error", title: "保存失败", message: String(err) });
          }
        })();
      },
    });
  }

  function bindEventRuleActions(container: HTMLElement, onClose?: () => void): void {
    container.querySelectorAll(".ev-open-rule-mgr, .ev-rule-jump").forEach((btn) => {
      btn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        const id = (btn as HTMLElement).dataset.evId!;
        const event = cachedEvents.find((x) => x.id === id);
        if (event) void openEventRuleManager(event, onClose);
      });
    });
  }

  function renderTriageDetailBody(t: TriageSummary, event: WafEvent): string {
    const evRows = (t.evidence ?? [])
      .map(
        (x) =>
          `<tr><td><code>${esc(x.signal)}</code></td><td>${x.weight > 0 ? "+" : ""}${x.weight}</td><td>${esc(x.detail)}</td></tr>`,
      )
      .join("");
    const aiBlock = t.ai_summary
      ? `<div class="triage-ai-block" style="margin-top:12px;padding:12px;background:var(--bg-soft,#f6ffed);border:1px solid var(--border,#b7eb8f);border-radius:6px">
           <strong>AI 解释</strong>
           <span class="muted small"> · ${esc(t.ai_engine ?? "ai")}${t.ai_model ? ` · ${esc(t.ai_model)}` : ""}</span>
           <div style="margin-top:8px;white-space:pre-wrap">${esc(t.ai_summary)}</div>
         </div>`
      : "";
    return `
      <div style="margin-bottom:12px;display:flex;align-items:center;gap:8px;flex-wrap:wrap">
        ${triageBadge(t.verdict, t.confidence)}
        <span class="muted">风险分 ${t.risk_score} · ${esc(t.engine)}</span>
      </div>
      <p>${esc(t.summary)}</p>
      ${renderEventRuleBlock(event)}
      ${aiBlock}
      <h4 class="small">证据链</h4>
      <table class="data-table"><thead><tr><th>信号</th><th>权重</th><th>说明</th></tr></thead><tbody>${evRows || "<tr><td colspan=3 class=muted>—</td></tr>"}</tbody></table>
      <h4 class="small" style="margin-top:12px">建议</h4>
      <ul>${(t.recommendations ?? []).map((r) => `<li>${esc(r)}</li>`).join("") || "<li class=muted>—</li>"}</ul>
      <p class="muted small">事件 ID: <code>${esc(event.id)}</code>${event.site_id ? ` · 应用: ${esc(siteDisplayName(siteList, event.site_id))} <code>${esc(event.site_id)}</code>` : ""}</p>`;
  }

  async function confirmFalsePositive(event: WafEvent): Promise<void> {
    const siteId = (event.site_id ?? "default").trim() || "default";
    const uri = event.uri ?? "—";
    const confirmed = await showConfirm({
      title: "确认误报并修复",
      message: `将为此事件自动加白路径${event.rule_id ? `并禁用规则 ${event.rule_id}` : ""}，确认继续？`,
      okText: "确认修复",
    });
    if (!confirmed) return;
    try {
      const res = await apiPost<{
        ok: boolean;
        applied: string[];
        audit_id: string;
      }>(`/api/events/${encodeURIComponent(event.id)}/confirm-false-positive`, {
        site_id: siteId,
        note: `Console 确认误报 ${uri}`,
      });
      await showResult({
        type: "success",
        title: "误报治理完成",
        message: (res.applied ?? []).join("\n") || "策略已更新",
      });
    } catch (e) {
      await showResult({
        type: "error",
        title: "误报修复失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  async function tempBanEventIp(event: WafEvent): Promise<void> {
    const ip = (event.src_ip ?? "").trim();
    if (!ip || ip === "—") {
      await showResult({ type: "error", title: "无法封禁", message: "事件缺少源 IP" });
      return;
    }
    await runMutatingAction({
      action: "save",
      subject: `临时封禁 ${ip}`,
      confirmMessage: `将源 IP ${ip} 写入运行时临时封禁（不改插件配置，可在访问控制中解除）。确认继续？`,
      successMessage: `${ip} 已临时封禁`,
      danger: true,
      run: async () => {
        await apiTempDeny(ip, "console-events");
      },
    });
  }

  async function whitelistEventIp(event: WafEvent): Promise<void> {
    const ip = (event.src_ip ?? "").trim();
    if (!ip || ip === "—") {
      await showResult({ type: "error", title: "无法加白", message: "事件缺少源 IP" });
      return;
    }
    await runMutatingAction({
      action: "save",
      subject: `加白源 IP ${ip}`,
      confirmMessage: `将 ${ip} 加入全局白名单（按源 IP 放行）。确认继续？`,
      successMessage: `${ip} 已加入白名单`,
      run: async () => {
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
      },
    });
  }

  async function openTriageModal(event: WafEvent): Promise<void> {
    hideModal();
    const mask = el("div", "ant-modal-mask");
    const wrap = el("div", "ant-modal-wrap");
    const modal = el("div", "ant-modal ant-modal-xl");
    const header = el("div", "ant-modal-header");
    header.innerHTML = "<span>智能研判详情</span>";
    const closeBtn = el("button", "ant-modal-close", "×");
    closeBtn.type = "button";
    const close = () => mask.remove();
    closeBtn.onclick = close;
    header.appendChild(closeBtn);

    const body = el("div", "ant-modal-body");
    body.innerHTML = `<div class="page-loading">加载研判信息…</div>`;

    const footer = el("div", "ant-modal-footer nerve-result-footer");
    const hasIp = !!(event.src_ip ?? "").trim();

    if (hasIp) {
      const banBtn = el("button", "ant-btn ant-btn-danger", "临时封禁源 IP");
      banBtn.type = "button";
      banBtn.onclick = async () => {
        await tempBanEventIp(event);
      };
      const wlBtn = el("button", "ant-btn", "加白源 IP");
      wlBtn.type = "button";
      wlBtn.onclick = async () => {
        await whitelistEventIp(event);
      };
      footer.append(banBtn, wlBtn);
    }

    const fpBtn = el("button", "ant-btn ant-btn-primary", "确认误报并修复");
    fpBtn.type = "button";
    fpBtn.onclick = async () => {
      await confirmFalsePositive(event);
      close();
    };

    const okBtn = el("button", "ant-btn", "知道了");
    okBtn.type = "button";
    okBtn.onclick = close;

    footer.append(fpBtn, okBtn);

    modal.append(header, body, footer);
    wrap.appendChild(modal);
    mask.appendChild(wrap);
    mask.addEventListener("click", (e) => {
      if (e.target === mask) close();
    });
    document.body.appendChild(mask);

    let triage = event.triage;
    try {
      if (!triage?.evidence) {
        const res = await apiPost<{ triage: TriageSummary }>("/api/triage/analyze", { event });
        triage = res.triage;
      }
      body.innerHTML = renderTriageDetailBody(triage!, event);
      bindEventRuleActions(body, close);
    } catch (e) {
      body.innerHTML = `<div class="ant-alert ant-alert-error">${esc(String(e))}</div>`;
    }
  }

  function renderAppCell(siteId: string | undefined, sites: ProtectedSite[]): string {
    const id = (siteId ?? "default").trim() || "default";
    const name = siteDisplayName(sites, id);
    return `<div class="ev-app-cell"><span class="app-event-tag">${esc(name)}</span><code class="small muted">${esc(id)}</code></div>`;
  }

  let siteList: ProtectedSite[] = [];
  try {
    siteList = await loadSites();
  } catch {
    siteList = [{ id: "default", name: "系统默认", hosts: ["*"], enabled: true }];
  }

  const initialSiteFilter = consumeEventsSiteFilter() ?? "";

  const siteFilterEl = root.querySelector("#ev-filter-site") as HTMLSelectElement;
  siteFilterEl.innerHTML = [
    `<option value="">全部应用</option>`,
    ...siteList.map(
      (s) =>
        `<option value="${esc(s.id)}">${esc(siteDisplayName(siteList, s.id))} (${esc(s.id)})</option>`,
    ),
  ].join("");
  if (initialSiteFilter && siteFilterEl.querySelector(`option[value="${CSS.escape(initialSiteFilter)}"]`)) {
    siteFilterEl.value = initialSiteFilter;
  }

  let evBackend = "";
  let evTotal = 0;
  const evPage: PaginationState = defaultPaginationState();

  function confidenceClass(confidence: number): string {
    if (confidence >= 80) return "ev-confidence ev-confidence-high";
    if (confidence >= 60) return "ev-confidence ev-confidence-mid";
    return "ev-confidence ev-confidence-low";
  }

  function inspectSourceLabel(source?: string): string {
    switch (source) {
      case "ebpf":
        return "eBPF";
      case "http-bypass":
        return "Gateway";
      case "waf-edge":
        return "WAF Edge";
      case "agent-local":
        return "Agent";
      default:
        return source?.trim() || "—";
    }
  }

  function stageLabel(stage?: string): string {
    switch (stage) {
      case "semantic":
        // Legacy events: semantic was a separate stage; display under core
        return "核心";
      case "expert":
        return "专家";
      case "core":
        return "核心";
      case "custom":
        return "自定义";
      case "pre_access":
        return "前置";
      default:
        return "";
    }
  }

  /** Legacy-event fallback: infer protection stage and rule set from plugin / category / message */
  function inferProtectionOrigin(e: WafEvent): { stage?: string; ruleSet?: string } {
    if (e.protection_stage || e.rule_set) {
      // Legacy protection_stage=semantic → display under core
      if (e.protection_stage === "semantic") {
        return { stage: "core", ruleSet: e.rule_set || "semantic" };
      }
      return { stage: e.protection_stage, ruleSet: e.rule_set };
    }
    const plugin = e.plugin_name || e.message?.match(/plugin=([^\s]+)/)?.[1];
    const engines = e.message?.match(/engines=([^\s]+)/)?.[1]?.split(",")?.[0];
    if (plugin === "nerve-semantic" || e.message?.includes("semantic_score=")) {
      return { stage: "core", ruleSet: engines || "semantic" };
    }
    if (plugin === "libinjection") return { stage: "core", ruleSet: "注入检测" };
    if (plugin === "expert" || plugin === "coraza" || e.category === "expert" || e.category === "crs") {
      return { stage: "expert", ruleSet: "expert-crs" };
    }
    if (plugin === "custom-rules" || e.category === "custom") {
      return { stage: "custom", ruleSet: "custom" };
    }
    if (plugin === "bot" || plugin === "cc" || plugin === "geo" || plugin === "behavior") {
      return { stage: "pre_access", ruleSet: plugin };
    }
    if (e.rule_id && !/^\d/.test(e.rule_id)) {
      return { stage: "core", ruleSet: "l7-json" };
    }
    return {};
  }

  function strategyCell(e: WafEvent): string {
    const name = e.strategy_name?.trim();
    if (!name) return `<span class="muted">—</span>`;
    const id = e.strategy_id?.trim();
    return id
      ? `<span title="${esc(id)}">${esc(name)}</span>`
      : esc(name);
  }

  /** Protection module = pipeline stage (core/expert/custom) · rule set (semantic means semantically enhanced) */
  function moduleCell(e: WafEvent): string {
    const inferred = inferProtectionOrigin(e);
    const rawStage = e.protection_stage === "semantic" ? "core" : e.protection_stage || inferred.stage;
    const stage = stageLabel(rawStage);
    const ruleSet = sanitizeProductText(e.rule_set || inferred.ruleSet || "").trim();
    if (!stage && !ruleSet) return `<span class="muted">—</span>`;
    if (stage && ruleSet) {
      return `<span class="layer-badge l7">${esc(stage)}</span><br><code class="muted small">${esc(ruleSet === "libinjection" ? "注入检测" : ruleSet)}</code>`;
    }
    return stage
      ? `<span class="layer-badge l7">${esc(stage)}</span>`
      : `<code class="small">${esc(ruleSet)}</code>`;
  }

  function eventRowHtml(e: WafEvent): string {
    const ip = e.src_ip ?? e.dst_ip ?? "—";
    const reqMsg =
      e.category === "l3_policy" && e.message
        ? `${e.message}${e.dst_ip && !e.message.includes(e.dst_ip) ? ` · ${e.dst_ip}` : ""}`
        : `${e.method ?? ""} ${e.uri ?? e.message ?? "—"}`.trim();
    const semanticHint = e.message?.includes("semantic_score=")
      ? `<br><span class="muted small">${esc(e.message ?? "")}</span>`
      : "";
    const sample = e.payload_sample ?? (e.dst_ip ? `deny_ip=${e.dst_ip}` : "");
    const triageCell = e.triage
      ? `<span class="${triageVerdictClass(e.triage.verdict)}">${esc(triageVerdictLabel(e.triage.verdict))}</span>`
      : `<span class="muted">—</span>`;
    const confidenceCell = e.triage
      ? `<span class="ev-confidence-cell"><span class="${confidenceClass(e.triage.confidence)}" title="研判引擎对「${esc(triageVerdictLabel(e.triage.verdict))}」结论的确信程度">${e.triage.confidence}%</span> <button type="button" class="ant-btn-link ant-btn-sm ev-triage-view" data-triage-view="${esc(e.id)}">查看</button>${e.triage.verdict === "likely_false_positive" ? ` <button type="button" class="ant-btn-link ant-btn-sm ev-fp-confirm" data-fp-confirm="${esc(e.id)}">确认误报</button>` : ""}</span>`
      : `<span class="muted">—</span>`;
    const hasIp = !!(e.src_ip ?? "").trim();
    const disposeCell = hasIp
      ? `<span class="ev-dispose-cell">
          <button type="button" class="ant-btn-link ant-btn-sm ev-temp-ban" data-temp-ban="${esc(e.id)}">封禁</button>
          <button type="button" class="ant-btn-link ant-btn-sm ev-wl-ip" data-wl-ip="${esc(e.id)}">加白</button>
        </span>`
      : `<span class="muted">—</span>`;
    return `<tr data-ev-id="${esc(e.id)}">
      <td><code class="small">${esc(formatTsEvent(e.ts))}</code></td>
      <td>${renderAppCell(e.site_id, siteList)}</td>
      <td>${actionTag(e.action)}</td>
      <td><span class="layer-badge l7">${esc(inspectSourceLabel(e.inspect_source))}</span></td>
      <td class="small">${strategyCell(e)}</td>
      <td class="small">${moduleCell(e)}</td>
      <td>${renderRuleCell(e)}</td>
      <td><span class="layer-badge l7">${esc(categoryLabel(e.category))}</span></td>
      <td><code>${esc(ip)}</code></td>
      <td><code class="small">${esc(reqMsg || "—")}${semanticHint}</code></td>
      <td class="muted small">${esc(sample.slice(0, 80))}</td>
      <td>${triageCell}</td>
      <td>${confidenceCell}</td>
      <td>${disposeCell}</td>
    </tr>`;
  }

  function bindTriageViewButtons(container: HTMLElement): void {
    container.querySelectorAll("[data-triage-view]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = (btn as HTMLElement).dataset.triageView!;
        const ev = cachedEvents.find((x) => x.id === id);
        if (ev) void openTriageModal(ev);
      });
    });
    container.querySelectorAll("[data-fp-confirm]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = (btn as HTMLElement).dataset.fpConfirm!;
        const ev = cachedEvents.find((x) => x.id === id);
        if (ev) void confirmFalsePositive(ev);
      });
    });
    container.querySelectorAll("[data-temp-ban]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = (btn as HTMLElement).dataset.tempBan!;
        const ev = cachedEvents.find((x) => x.id === id);
        if (ev) void tempBanEventIp(ev);
      });
    });
    container.querySelectorAll("[data-wl-ip]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const id = (btn as HTMLElement).dataset.wlIp!;
        const ev = cachedEvents.find((x) => x.id === id);
        if (ev) void whitelistEventIp(ev);
      });
    });
  }

  function paintEventsTable(): void {
    const table = root.querySelector("#ev-table")!;
    if (!cachedEvents.length) {
      table.innerHTML = `<div class="ant-empty">暂无事件（backend: ${esc(evBackend)}）</div>`;
      return;
    }
    const section = renderPaginatedSection("ev-list", cachedEvents, evPage, (pageEvents) => {
      const rows = pageEvents.map((e) => eventRowHtml(e)).join("");
      return `<div class="muted table-summary">共 ${evTotal} 条 · 存储: ${esc(evBackend)}</div>
        <div class="ant-table-wrapper">
        <table class="ant-table"><thead><tr>
          <th>时间</th><th>应用</th><th>动作</th><th>流量入口</th><th>防护策略</th><th>防护模块</th><th>命中规则</th><th>类型</th><th>相关 IP</th><th>请求/消息</th><th>样本</th><th>智能研判</th><th title="研判引擎对结论的确信程度（0–100，越高越可信）">研判置信度</th><th>处置</th>
        </tr></thead><tbody>${rows}</tbody></table>
        </div>`;
    });
    evPage.page = section.page;
    table.innerHTML = section.html;
    bindPaginationBar(table, "ev-list", evPage, cachedEvents.length, (next) => {
      evPage.page = next.page;
      evPage.pageSize = next.pageSize;
      paintEventsTable();
    });
    bindTriageViewButtons(table);
    bindEventRuleActions(table);
  }

  async function load(): Promise<void> {
    const cat = (root.querySelector("#ev-filter-cat") as HTMLSelectElement).value;
    const siteId = (root.querySelector("#ev-filter-site") as HTMLSelectElement).value;
    const q = new URLSearchParams({ limit: "100", triage: "true", layer: "l7" });
    if (cat) q.set("category", cat);
    if (siteId) q.set("site_id", siteId);
    const data = await apiGet<{ total: number; events: WafEvent[]; backend: string }>(`/api/events?${q}`);
    cachedEvents = data.events;
    evBackend = data.backend;
    evTotal = data.total;
    evPage.page = 1;
    paintEventsTable();
  }

  root.querySelector("#btn-refresh-ev")?.addEventListener("click", () => void load().catch((e) => toast(String(e), false)));
  root.querySelector("#ev-filter-cat")?.addEventListener("change", () => void load().catch((e) => toast(String(e), false)));
  root.querySelector("#ev-filter-site")?.addEventListener("change", () => void load().catch((e) => toast(String(e), false)));
  await load();
}

export async function renderProtectionModules(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const meta = PAGE_META.modules;
  let siteId = GLOBAL_DEFAULT_SITE_ID;

  root.innerHTML = `
    <div class="page-header">
      <div><h2>${meta.title}</h2><p>${meta.subtitle}</p></div>
    </div>
    <div class="table-toolbar">
      <span class="muted">全局默认模板 <code>${GLOBAL_DEFAULT_SITE_ID}</code></span>
      <label style="margin-left:16px">模式
        <select class="ant-input" id="mod-mode" style="width:140px;margin-left:8px">
          <option value="block">拦截模式</option>
          <option value="detect">检测模式（仅记录）</option>
        </select>
      </label>
      <button type="button" class="ant-btn ant-btn-primary" id="btn-save-mod">发布</button>
    </div>
    <div class="module-grid" id="mod-grid">加载中…</div>`;

  const grid = root.querySelector("#mod-grid")!;
  let modules = await apiGet<ProtectionModules>("/api/global/modules");
  (root.querySelector("#mod-mode") as HTMLSelectElement).value = modules.mode;

  function renderGrid(): void {
    grid.innerHTML = "";
    for (const [key, cfg] of orderedWebCoreModuleEntries(modules.modules)) {
      const modMeta = MODULE_META[key] ?? { title: key, desc: "" };
      const crsCat = MODULE_CRS_CATEGORY[key];
      const viewRulesBtn = crsCat
        ? `<button type="button" class="ant-btn ant-btn-link ant-btn-sm mod-view-rules">查看规则</button>`
        : "";
      const card = el("div", `protection-module-card app-mod-card${cfg.enabled ? "" : " mod-card-disabled"}`);
      card.dataset.key = key;
      if (crsCat) card.dataset.crsCat = crsCat;
      card.innerHTML = `
      <div class="protection-module-head">
        <strong>${esc(modMeta.title)}</strong>
        <div class="protection-module-head-actions">
          ${viewRulesBtn}
          <label class="switch"><input type="checkbox" class="mod-enabled" ${cfg.enabled ? "checked" : ""}><span></span></label>
        </div>
      </div>
      <p class="muted">${esc(modMeta.desc)}</p>
      <div class="protection-module-actions">
        <select class="ant-input mod-action">
          <option value="block" ${cfg.action === "block" ? "selected" : ""}>拦截</option>
          <option value="log" ${cfg.action === "log" ? "selected" : ""}>仅记录</option>
          <option value="challenge" ${cfg.action === "challenge" ? "selected" : ""}>挑战</option>
        </select>
        <select class="ant-input mod-sensitivity">
          <option value="low" ${cfg.sensitivity === "low" ? "selected" : ""}>低</option>
          <option value="medium" ${cfg.sensitivity === "medium" ? "selected" : ""}>中</option>
          <option value="high" ${cfg.sensitivity === "high" ? "selected" : ""}>高</option>
        </select>
      </div>`;
      grid.appendChild(card);
    }

    bindModuleCardStyles(root);

    grid.querySelectorAll<HTMLButtonElement>(".mod-view-rules").forEach((btn) => {
      btn.onclick = () => {
        const card = btn.closest<HTMLElement>(".protection-module-card")!;
        const cat = card.dataset.crsCat;
        if (!cat) return;
        const key = card.dataset.key!;
        const modMeta = MODULE_META[key] ?? { title: key, desc: "" };
        const sens = (card.querySelector(".mod-sensitivity") as HTMLSelectElement).value;
        const enabled = (card.querySelector(".mod-enabled") as HTMLInputElement).checked;
        void showModuleRulesModal({
          category: cat,
          protect: sensitivityToProtectLevel(sens),
          moduleTitle: modMeta.title,
          moduleEnabled: enabled,
          moduleKey: key,
          siteId,
        });
      };
    });
  }

  renderGrid();

  root.querySelector("#btn-save-mod")?.addEventListener("click", async () => {
    const mode = (root.querySelector("#mod-mode") as HTMLSelectElement).value;
    const modeLabel = mode === "block" ? "拦截模式" : "检测模式（仅记录）";
    const ok = await showConfirm({
      title: "发布全局默认防护",
      message: `确认发布全局默认 Web 核心防护？\n\n模式：${modeLabel}\n未单独配置的应用将继承此模板。`,
      okText: "确认发布",
    });
    if (!ok) return;

    const next: ProtectionModules = {
      version: modules.version,
      mode,
      modules: {},
    };
    grid.querySelectorAll<HTMLElement>(".protection-module-card").forEach((card) => {
      const key = card.dataset.key!;
      const base = modules.modules[key];
      const cfg: ModuleConfig = {
        enabled: (card.querySelector(".mod-enabled") as HTMLInputElement).checked,
        action: (card.querySelector(".mod-action") as HTMLSelectElement).value,
        sensitivity: (card.querySelector(".mod-sensitivity") as HTMLSelectElement).value,
      };
      if (base?.disabled_rules?.length) cfg.disabled_rules = [...base.disabled_rules];
      next.modules[key] = cfg;
    });
    // Keep extension module (custom / plugin / crs) config; not shown on this page
    for (const key of ["custom", "plugin", "crs"] as const) {
      if (modules.modules[key]) {
        next.modules[key] = { ...modules.modules[key] };
      }
    }
    const btn = root.querySelector("#btn-save-mod") as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "发布中…";
    try {
      await apiPut("/api/global/modules", next);
      modules = next;
      void apiPost("/api/sync").catch(() => null);
      toast("全局默认防护配置已发布");
    } catch (e) {
      toast(String(e), false);
    } finally {
      btn.disabled = false;
      btn.textContent = "发布";
    }
  });
}
