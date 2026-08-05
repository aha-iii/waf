/** 语义 NSP 配置 UI — 全局「语义设置」与应用「防护设置 → 语义设置」共用 */

import { esc, switchHtml } from "./components/ui.js";

export type DecodePolicy = {
  max_depth: number;
  max_expand_bytes: number;
  url: boolean;
  html_entity: boolean;
  base64: boolean;
  hex: boolean;
  unicode: boolean;
  chr: boolean;
  order: string[];
};

export type DetectionScope = {
  query: boolean;
  body: boolean;
  header: boolean;
  cookie: boolean;
  path: boolean;
};

export type SemanticWhitelist = {
  param_names: string[];
  path_prefixes: string[];
  content_types: string[];
};

export type EngineFusion = {
  semantic_first: boolean;
  libinjection: boolean;
  sql_intent: boolean;
  fallback_plain_libinjection: boolean;
  pipeline_order: string[];
  pipeline_enabled?: Record<string, boolean>;
};

export type SemanticPerformance = {
  max_slices_per_request: number;
  slow_path_sample_rate: number;
  timeoutttttttttttttttttttttt_ms: number;
};

export type SemanticAutoTune = {
  enabled: boolean;
  false_positive_threshold: number;
  auto_whitelist_param: boolean;
  auto_relax_mode: boolean;
  relax_after_fp_count: number;
  window_hours: number;
};

export type SemanticConfig = {
  enabled: boolean;
  mode: "strict" | "standard" | "relaxed";
  decode: DecodePolicy;
  scope: DetectionScope;
  whitelist: SemanticWhitelist;
  engines: EngineFusion;
  performance: SemanticPerformance;
  auto_tune: SemanticAutoTune;
  intent_enabled: boolean;
  /** 应用侧省略/undefined = 继承全局 */
  expert_level?: "low" | "medium" | "high";
};

export type SemanticStats = {
  site_id?: string;
  backend?: string;
  counters: Record<string, number>;
  hit_rate_pct: number;
  recent_samples: {
    ts: string;
    param_path: string;
    decode_chain: string[];
    semantic_score: number;
    engines: string[];
    uri: string;
    hit: boolean;
  }[];
  feedback: {
    ts: string;
    event_id?: string;
    verdict: string;
    note?: string;
    applied_action?: string;
  }[];
};

export type SemanticTab =
  | "general"
  | "decode"
  | "scope"
  | "whitelist"
  | "engines"
  | "performance"
  | "autotune"
  | "observe";

export const DECODE_OPS = ["url", "html_entity", "base64", "hex", "unicode", "chr"] as const;

export const SEMANTIC_NAV_ITEMS: { id: SemanticTab; icon: string; label: string; desc: string }[] = [
  { id: "general", icon: "⚙", label: "基础设置", desc: "站点 · 总开关 · 语义模式" },
  { id: "decode", icon: "🔓", label: "解码策略", desc: "编码开关 · 顺序 · 递归深度" },
  { id: "scope", icon: "🎯", label: "检测范围", desc: "Query / Body / Header 等" },
  { id: "whitelist", icon: "✓", label: "业务白名单", desc: "参数 · 路径 · Content-Type" },
  { id: "engines", icon: "🔀", label: "引擎组合", desc: "语义 · 流水线顺序" },
  { id: "performance", icon: "⏱", label: "性能", desc: "切片上限 · 超时 · 采样" },
  { id: "autotune", icon: "🔄", label: "自动调优", desc: "误报反馈闭环" },
  { id: "observe", icon: "📊", label: "观测", desc: "命中率 · 解码链 · 反馈" },
];

export function defaultSemanticConfig(): SemanticConfig {
  return {
    enabled: true,
    mode: "standard",
    decode: {
      max_depth: 8,
      max_expand_bytes: 65536,
      url: true,
      html_entity: true,
      base64: true,
      hex: false,
      unicode: false,
      chr: false,
      order: [...DECODE_OPS],
    },
    scope: { query: true, body: true, header: true, cookie: true, path: true },
    whitelist: { param_names: [], path_prefixes: [], content_types: [] },
    engines: {
      semantic_first: true,
      libinjection: true,
      sql_intent: true,
      fallback_plain_libinjection: true,
      pipeline_order: ["expert", "core", "custom"],
    },
    performance: {
      max_slices_per_request: 128,
      slow_path_sample_rate: 100,
      timeoutttttttttttttttttttttt_ms: 50,
    },
    auto_tune: {
      enabled: true,
      false_positive_threshold: 3,
      auto_whitelist_param: true,
      auto_relax_mode: true,
      relax_after_fp_count: 10,
      window_hours: 168,
    },
    intent_enabled: true,
  };
}

export function mergeSemanticConfig(raw: Partial<SemanticConfig> | undefined): SemanticConfig {
  const d = defaultSemanticConfig();
  if (!raw) return d;
  let enabled = raw.enabled ?? d.enabled;
  if (raw.engines?.pipeline_enabled && "semantic" in raw.engines.pipeline_enabled
      && raw.engines.pipeline_enabled.semantic === false) {
    enabled = false;
  }
  const orderRaw = raw.engines?.pipeline_order ?? d.engines.pipeline_order;
  return {
    ...d,
    ...raw,
    enabled,
    decode: { ...d.decode, ...raw.decode },
    scope: { ...d.scope, ...raw.scope },
    whitelist: { ...d.whitelist, ...raw.whitelist },
    engines: {
      ...d.engines,
      ...raw.engines,
      pipeline_order: orderRaw.filter((id) => id !== "semantic" && id !== "plugin"),
    },
    performance: { ...d.performance, ...raw.performance },
    auto_tune: { ...d.auto_tune, ...raw.auto_tune },
  };
}

export function semanticConfigSnapshot(cfg: SemanticConfig): string {
  return JSON.stringify({
    enabled: cfg.enabled,
    mode: cfg.mode,
    decode: cfg.decode,
    scope: cfg.scope,
    whitelist: cfg.whitelist,
    engines: {
      semantic_first: cfg.engines.semantic_first,
      libinjection: cfg.engines.libinjection,
      sql_intent: cfg.engines.sql_intent,
      fallback_plain_libinjection: cfg.engines.fallback_plain_libinjection,
      pipeline_order: cfg.engines.pipeline_order,
    },
    performance: cfg.performance,
    auto_tune: cfg.auto_tune,
    intent_enabled: cfg.intent_enabled,
    expert_level: cfg.expert_level ?? null,
  });
}

function linesToList(s: string): string[] {
  return s
    .split("\n")
    .map((x) => x.trim())
    .filter(Boolean);
}

function listToLines(arr: string[] | undefined): string {
  return (arr ?? []).join("\n");
}

function chk(id: string, on: boolean, label: string): string {
  return `<label class="sem-check-row">${esc(label)} <input type="checkbox" id="${id}" ${on ? "checked" : ""} /></label>`;
}

function decodeOrderHtml(order: string[]): string {
  return order
    .map(
      (op, i) =>
        `<div class="order-row" data-op="${esc(op)}">
          <span class="muted">${i + 1}.</span> <code>${esc(op)}</code>
          <button type="button" class="ant-btn ant-btn-sm btn-up" data-i="${i}">↑</button>
          <button type="button" class="ant-btn ant-btn-sm btn-down" data-i="${i}">↓</button>
        </div>`,
    )
    .join("");
}

function readOrder(scope: HTMLElement): string[] {
  const orderEl = scope.querySelector("#decode-order");
  if (!orderEl) return defaultSemanticConfig().decode.order;
  return [...orderEl.querySelectorAll(".order-row")].map((r) => (r as HTMLElement).dataset.op ?? "");
}

export function collectSemanticConfig(scope: HTMLElement, base: SemanticConfig): SemanticConfig {
  const q = (id: string) => scope.querySelector(`#${id}`) as HTMLInputElement | null;
  const ta = (id: string) => scope.querySelector(`#${id}`) as HTMLTextAreaElement | null;
  const sel = (id: string) => scope.querySelector(`#${id}`) as HTMLSelectElement | null;

  const cfg: SemanticConfig = {
    ...base,
    decode: { ...base.decode },
    scope: { ...base.scope },
    whitelist: { ...base.whitelist },
    engines: { ...base.engines },
    performance: { ...base.performance },
    auto_tune: { ...base.auto_tune },
  };

  const enabled = q("sem-enabled");
  if (enabled) cfg.enabled = enabled.checked;

  const mode = sel("sem-mode");
  if (mode) cfg.mode = mode.value as SemanticConfig["mode"];

  if (q("dec-url")) {
    cfg.decode = {
      max_depth: Number(q("sem-depth")?.value ?? cfg.decode.max_depth),
      max_expand_bytes: Number(q("sem-bytes")?.value ?? cfg.decode.max_expand_bytes),
      url: q("dec-url")!.checked,
      html_entity: q("dec-html")!.checked,
      base64: q("dec-b64")!.checked,
      hex: q("dec-hex")!.checked,
      unicode: q("dec-uni")!.checked,
      chr: q("dec-chr")!.checked,
      order: readOrder(scope),
    };
  }

  if (q("scope-query")) {
    cfg.scope = {
      query: q("scope-query")!.checked,
      body: q("scope-body")!.checked,
      header: q("scope-header")!.checked,
      cookie: q("scope-cookie")!.checked,
      path: q("scope-path")!.checked,
    };
  }

  if (ta("wl-params")) {
    cfg.whitelist = {
      param_names: linesToList(ta("wl-params")!.value),
      path_prefixes: linesToList(ta("wl-paths")!.value),
      content_types: linesToList(ta("wl-ct")!.value),
    };
  }

  if (q("eng-sem-first")) {
    cfg.engines = {
      ...cfg.engines,
      semantic_first: q("eng-sem-first")!.checked,
      libinjection: q("eng-li")!.checked,
      sql_intent: q("eng-intent")!.checked,
      fallback_plain_libinjection: q("eng-fallback")!.checked,
      pipeline_order: (q("eng-order")?.value ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean),
    };
    cfg.intent_enabled = q("eng-intent")!.checked;
  }

  if (q("perf-slices")) {
    cfg.performance = {
      max_slices_per_request: Number(q("perf-slices")!.value),
      timeoutttttttttttttttttttttt_ms: Number(q("perf-timeoutttttttttttttttttttttt")!.value),
      slow_path_sample_rate: Number(q("perf-sample")!.value),
    };
  }

  if (q("at-enabled")) {
    cfg.auto_tune = {
      enabled: q("at-enabled")!.checked,
      false_positive_threshold: Number(q("at-fp-th")!.value),
      auto_whitelist_param: q("at-wl")!.checked,
      auto_relax_mode: q("at-relax")!.checked,
      relax_after_fp_count: Number(q("at-relax-n")!.value),
      window_hours: Number(q("at-window")!.value),
    };
  }

  return cfg;
}

function panelWrap(title: string, desc: string, body: string): string {
  return `
    <div class="alert-panel-head"><h3>${title}</h3>${desc ? `<p class="muted">${desc}</p>` : ""}</div>
    <div class="alert-panel-body form-stack">${body}</div>`;
}

function renderGeneralPanel(cfg: SemanticConfig): string {
  return panelWrap(
    "基础设置",
    "",
    `
    <label class="ant-form-label">语义模式</label>
    <select class="ant-input ant-input-wide" id="sem-mode">
      <option value="strict" ${cfg.mode === "strict" ? "selected" : ""}>严格</option>
      <option value="standard" ${cfg.mode === "standard" ? "selected" : ""}>标准</option>
      <option value="relaxed" ${cfg.mode === "relaxed" ? "selected" : ""}>宽松</option>
    </select>`,
  );
}

function renderDecodePanel(cfg: SemanticConfig): string {
  return panelWrap(
    "解码策略",
    "",
    `
    <div class="route-form-grid-2">
      ${chk("dec-url", cfg.decode.url, "URL 解码")}
      ${chk("dec-html", cfg.decode.html_entity, "HTML Entity")}
      ${chk("dec-b64", cfg.decode.base64, "Base64")}
      ${chk("dec-hex", cfg.decode.hex, "Hex（\\x / 0x）")}
      ${chk("dec-uni", cfg.decode.unicode, "Unicode（\\u / %u）")}
      ${chk("dec-chr", cfg.decode.chr, "CHR（char/chr）")}
    </div>
    <label class="ant-form-label">解码顺序</label>
    <div id="decode-order">${decodeOrderHtml(cfg.decode.order)}</div>
    <label class="ant-form-label">递归最大层数</label>
    <input type="number" class="ant-input ant-input-wide" id="sem-depth" min="1" max="32" value="${cfg.decode.max_depth}" />
    <label class="ant-form-label">单参数最大展开字节</label>
    <input type="number" class="ant-input ant-input-wide" id="sem-bytes" min="1024" step="1024" value="${cfg.decode.max_expand_bytes}" />`,
  );
}

function renderScopePanel(cfg: SemanticConfig): string {
  return panelWrap(
    "检测范围",
    "",
    `
    <div class="route-form-grid-2">
      ${chk("scope-query", cfg.scope.query, "Query 参数")}
      ${chk("scope-body", cfg.scope.body, "Request Body")}
      ${chk("scope-header", cfg.scope.header, "Header")}
      ${chk("scope-cookie", cfg.scope.cookie, "Cookie")}
      ${chk("scope-path", cfg.scope.path, "路径段")}
    </div>`,
  );
}

function renderWhitelistPanel(cfg: SemanticConfig): string {
  return panelWrap(
    "业务白名单",
    "",
    `
    <label class="ant-form-label">跳过参数名（每行一个）</label>
    <textarea class="ant-input ant-input-wide" id="wl-params" rows="3" placeholder="csrf_token">${esc(listToLines(cfg.whitelist.param_names))}</textarea>
    <label class="ant-form-label">路径前缀（每行一个）</label>
    <textarea class="ant-input ant-input-wide" id="wl-paths" rows="3" placeholder="/static">${esc(listToLines(cfg.whitelist.path_prefixes))}</textarea>
    <label class="ant-form-label">Content-Type 例外（body 跳过）</label>
    <textarea class="ant-input ant-input-wide" id="wl-ct" rows="2" placeholder="text/plain">${esc(listToLines(cfg.whitelist.content_types))}</textarea>`,
  );
}

function renderEnginesPanel(cfg: SemanticConfig): string {
  return panelWrap(
    "引擎组合",
    "",
    `
    ${chk("eng-sem-first", cfg.engines.semantic_first, "核心段内优先跑语义增强")}
    ${chk("eng-li", cfg.engines.libinjection, "语义解码链内嵌检测")}
    ${chk("eng-intent", cfg.engines.sql_intent, "SQL 意图慢路径")}
    ${chk("eng-fallback", cfg.engines.fallback_plain_libinjection, "增强未命中时回退 plain 核心检测")}
    <label class="ant-form-label">流水线顺序（逗号分隔）</label>
    <input class="ant-input ant-input-wide" id="eng-order" value="${esc(cfg.engines.pipeline_order.join(","))}" />`,
  );
}

function renderPerformancePanel(cfg: SemanticConfig): string {
  return panelWrap(
    "性能",
    "",
    `
    <label class="ant-form-label">单请求最大切片数</label>
    <input type="number" class="ant-input ant-input-wide" id="perf-slices" min="1" max="512" value="${cfg.performance.max_slices_per_request}" />
    <label class="ant-form-label">语义检测超时（毫秒）</label>
    <input type="number" class="ant-input ant-input-wide" id="perf-timeoutttttttttttttttttttttt" min="5" max="500" value="${cfg.performance.timeoutttttttttttttttttttttt_ms}" />
    <label class="ant-form-label">SQL 意图慢路径采样率（0–100%）</label>
    <input type="number" class="ant-input ant-input-wide" id="perf-sample" min="0" max="100" value="${cfg.performance.slow_path_sample_rate}" />`,
  );
}

function renderAutotunePanel(cfg: SemanticConfig): string {
  return panelWrap(
    "反馈驱动调优",
    "",
    `
    ${chk("at-enabled", cfg.auto_tune.enabled, "启用自动调优")}
    ${chk("at-wl", cfg.auto_tune.auto_whitelist_param, "同参数误报达阈值 → 自动加参数白名单")}
    <label class="ant-form-label">单参数误报阈值（次）</label>
    <input type="number" class="ant-input ant-input-wide" id="at-fp-th" min="1" value="${cfg.auto_tune.false_positive_threshold}" />
    ${chk("at-relax", cfg.auto_tune.auto_relax_mode, "窗口内误报过多 → 自动切 relaxed")}
    <label class="ant-form-label">触发 relaxed 的误报总数</label>
    <input type="number" class="ant-input ant-input-wide" id="at-relax-n" min="1" value="${cfg.auto_tune.relax_after_fp_count}" />
    <label class="ant-form-label">统计窗口（小时）</label>
    <input type="number" class="ant-input ant-input-wide" id="at-window" min="1" max="720" value="${cfg.auto_tune.window_hours}" />`,
  );
}

function renderObservePanel(stats: SemanticStats | null, backend: string): string {
  if (!stats) {
    return panelWrap("观测", "", `<p class="muted">统计暂不可用</p>`);
  }
  const samples =
    (stats.recent_samples ?? [])
      .slice(0, 10)
      .map(
        (s) =>
          `<tr><td>${esc(s.ts)}</td><td><code>${esc(s.uri.slice(0, 36))}</code></td><td>${esc(s.param_path)}</td><td>${s.semantic_score}</td><td><code>${esc(s.decode_chain.join(" → "))}</code></td></tr>`,
      )
      .join("") || `<tr><td colspan="5" class="muted">暂无样例，触发检测后会出现</td></tr>`;

  const feedback =
    (stats.feedback ?? [])
      .slice(0, 8)
      .map(
        (f) =>
          `<li>${esc(f.ts)} · ${esc(f.verdict)}${f.applied_action ? ` · <strong>${esc(f.applied_action)}</strong>` : ""}${f.note ? ` — ${esc(f.note)}` : ""}</li>`,
      )
      .join("") || "";

  return `
    <div class="alert-panel-head"><h3>观测</h3></div>
    <div class="alert-panel-body">
      <p>检测 <strong>${stats.counters.inspect_total ?? 0}</strong> 次 · 命中 <strong>${stats.counters.hits ?? 0}</strong> · 命中率 <strong>${stats.hit_rate_pct ?? 0}%</strong>
         · 白名单跳过 ${stats.counters.skipped_whitelist ?? 0} · 超时 ${stats.counters.skipped_timeoutttttttttttttttttttttt ?? 0}</p>
      <h4>最近解码链样例</h4>
      <div class="table-wrap"><table class="ant-table"><thead><tr><th>时间</th><th>URI</th><th>参数</th><th>分数</th><th>解码链</th></tr></thead><tbody>${samples}</tbody></table></div>
      <h4 style="margin-top:16px">提交反馈</h4>
      <div class="form-stack" style="max-width:480px">
        <input class="ant-input" id="fb-event" placeholder="事件 ID（可选，自动解析 param/uri）" />
        <select class="ant-input" id="fb-verdict">
          <option value="false_positive">误报</option>
          <option value="true_positive">确认攻击</option>
          <option value="uncertain">不确定</option>
        </select>
        <input class="ant-input" id="fb-note" placeholder="备注" />
        <button type="button" class="ant-btn ant-btn-primary" id="btn-fb">提交反馈</button>
      </div>
      ${feedback ? `<ul class="muted small" style="margin-top:12px">${feedback}</ul>` : ""}
    </div>`;
}

export function renderSemanticPanel(
  tab: SemanticTab,
  cfg: SemanticConfig,
  stats: SemanticStats | null,
  backend: string,
): string {
  switch (tab) {
    case "general":
      return renderGeneralPanel(cfg);
    case "decode":
      return renderDecodePanel(cfg);
    case "scope":
      return renderScopePanel(cfg);
    case "whitelist":
      return renderWhitelistPanel(cfg);
    case "engines":
      return renderEnginesPanel(cfg);
    case "performance":
      return renderPerformancePanel(cfg);
    case "autotune":
      return renderAutotunePanel(cfg);
    case "observe":
      return renderObservePanel(stats, backend);
    default:
      return "";
  }
}

export function renderSemanticNav(activeTab: SemanticTab, navAttr = "data-sem-tab"): string {
  return SEMANTIC_NAV_ITEMS.map(
    (item) =>
      `<button type="button" class="alert-nav-item${activeTab === item.id ? " active" : ""}" ${navAttr}="${item.id}">
        <span class="alert-nav-icon">${item.icon}</span>
        <span class="alert-nav-text">
          <strong>${item.label}</strong>
          <span class="muted small">${item.desc}</span>
        </span>
      </button>`,
  ).join("");
}

export type SemanticConsoleOptions = {
  activeTab: SemanticTab;
  cfg: SemanticConfig;
  stats?: SemanticStats | null;
  backend?: string;
  compact?: boolean;
  toolbarLeft?: string;
  toolbarRight?: string;
  footer?: string;
  navId?: string;
  panelId?: string;
  navAttr?: string;
};

export function renderSemanticConsole(opts: SemanticConsoleOptions): string {
  const {
    activeTab,
    cfg,
    stats = null,
    backend = "",
    compact = false,
    toolbarLeft = "",
    toolbarRight = "",
    footer = "",
    navId = "sem-nav",
    panelId = "sem-panel",
    navAttr = "data-sem-tab",
  } = opts;

  return `
    <div class="ant-card alert-console${compact ? " alert-console-compact" : ""}" id="app-sem-console">
      <div class="alert-console-toolbar">
        ${toolbarLeft}
        ${switchHtml("sem-enabled", cfg.enabled, "启用 NSP")}
        <span class="alert-toolbar-spacer"></span>
        ${toolbarRight}
      </div>
      <div class="alert-console-body">
        <nav class="alert-console-nav" id="${navId}">${renderSemanticNav(activeTab, navAttr)}</nav>
        <div class="alert-console-panel" id="${panelId}">${renderSemanticPanel(activeTab, cfg, stats, backend)}</div>
      </div>
      ${footer ? `<div class="alert-console-footer muted small">${footer}</div>` : ""}
    </div>`;
}

export function bindSemanticDecodeOrder(scope: HTMLElement, onSync: () => void): void {
  const orderEl = scope.querySelector("#decode-order");
  orderEl?.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (!t.classList.contains("btn-up") && !t.classList.contains("btn-down")) return;
    const rows = [...orderEl.querySelectorAll(".order-row")];
    const i = Number(t.dataset.i);
    const j = t.classList.contains("btn-up") ? i - 1 : i + 1;
    if (j < 0 || j >= rows.length) return;
    const parent = rows[0].parentElement!;
    if (t.classList.contains("btn-up")) parent.insertBefore(rows[i], rows[j]);
    else parent.insertBefore(rows[j], rows[i]);
    [...parent.querySelectorAll(".order-row")].forEach((row, idx) => {
      row.querySelector(".btn-up")?.setAttribute("data-i", String(idx));
      row.querySelector(".btn-down")?.setAttribute("data-i", String(idx));
      const span = row.querySelector(".muted");
      if (span) span.textContent = `${idx + 1}.`;
    });
    onSync();
  });
}
