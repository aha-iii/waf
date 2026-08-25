/** Legacy console module: App config drawer. */
import { apiGet, apiPost, apiPut, apiDelete } from "./api.js";
import {
  bindAppCrsVersionBar,
  fetchAppCrsStatus,
  renderAppCrsVersionBar,
  type AppCrsStatus,
} from "./app-crs-version.js";
import { esc, runMutatingAction, showPrompt, showResult, switchHtml, btn, emptyState } from "./components/ui.js";
import {
  CUSTOM_MODULE_KEY_PREFIX,
  MODULE_META,
  buildCustomModuleEntries,
  moduleCfgFromRecord,
  orderedWebCoreModuleEntries,
  type CustomRuleSetInfo,
  type ExtensionModuleKey,
  type ModuleCardEntry,
} from "./module-meta.js";
import { showCustomSetRulesModal } from "./custom-set-rules-modal.js";
import {
  siteQuery,
  type ProtectedSite,
} from "./sites.js";
import { bindModuleCardStyles } from "./module-display.js";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "./crs-preset.js";
import { showModuleRulesModal } from "./module-rules-modal.js";
import type { ProtectionModules } from "./security-view.js";
import {
  bindPipelineOrder,
  defaultPipelineEnabled,
  normalizePipelineEnabled,
  normalizePipelineOrder,
  readPipelineEnabled,
  readPipelineOrder,
  renderPipelineOrderList,
  type PipelineEnabledMap,
} from "./pipeline-stages.js";
import {
  bindSemanticDecodeOrder,
  collectSemanticConfig,
  mergeSemanticConfig as mergeSemanticBase,
  renderSemanticConsole,
  semanticConfigSnapshot,
  type SemanticConfig as SemanticConfigCore,
  type SemanticStats,
  type SemanticTab,
} from "./semantic-config-ui.js";
import {
  bindPipelineFlowEditor,
  bindPipelineModeSwitch,
  compactPipelineGraph,
  defaultPipelineEditor,
  destroyPipelineFlowEditor,
  getPipelineEditorMode,
  mergePipelineEditor,
  readPipelineFlowState,
  rebuildPipelineGraph,
  renderPipelineFlowEditor,
  type PipelineEditorState,
} from "./pipeline-flow.js";
import {
  fetchStrategyTemplates,
  isAppPrivateStrategy,
  isStrategyEnabled,
  strategyScopeLabel,
  type StrategyTemplate,
} from "./protection-levels-view.js";

type AppConfigTab = "basic" | "strategy" | "pipeline";

type SemanticConfig = SemanticConfigCore & {
  pipeline_editor?: PipelineEditorState;
};

function defaultSemanticConfig(): SemanticConfig {
  const base = mergeSemanticBase(undefined);
  return {
    ...base,
    engines: {
      ...base.engines,
      pipeline_enabled: defaultPipelineEnabled(),
    },
    pipeline_editor: defaultPipelineEditor(),
  };
}

function mergeSemanticConfig(raw: Partial<SemanticConfig> | undefined): SemanticConfig {
  const d = defaultSemanticConfig();
  if (!raw) return d;
  const merged = mergeSemanticBase(raw);
  // Legacy: pipeline_enabled.semantic=false → disable semantic enhancement
  const enabled =
    raw.engines?.pipeline_enabled && "semantic" in (raw.engines.pipeline_enabled as object)
      && (raw.engines.pipeline_enabled as PipelineEnabledMap).semantic === false
      ? false
      : merged.enabled;
  return {
    ...merged,
    enabled,
    engines: {
      ...merged.engines,
      pipeline_order: normalizePipelineOrder(
        merged.engines.pipeline_order ?? d.engines.pipeline_order ?? [],
      ),
      pipeline_enabled: normalizePipelineEnabled(
        raw.engines?.pipeline_enabled ?? d.engines.pipeline_enabled,
      ),
    },
    pipeline_editor: mergePipelineEditor(raw.pipeline_editor ?? d.pipeline_editor),
  };
}

function validateSiteId(site: ProtectedSite, allSites: ProtectedSite[], currentIndex: number): string | null {
  const id = site.id.trim();
  if (!id) return "应用 ID 不能为空";
  if (/\s/.test(id)) return "应用 ID 不能含空白字符";
  const dup = allSites.findIndex((s, i) => i !== currentIndex && s.id.trim() === id);
  if (dup >= 0) return `应用 ID「${id}」已存在，请更换`;
  return null;
}

function moduleEquals(
  a: ProtectionModules["modules"][string] | undefined,
  b: ProtectionModules["modules"][string] | undefined,
): boolean {
  if (!a && !b) return true;
  if (!a || !b) return false;
  const drA = [...(a.disabled_rules ?? [])].sort().join(",");
  const drB = [...(b.disabled_rules ?? [])].sort().join(",");
  return a.enabled === b.enabled && a.action === b.action && a.sensitivity === b.sensitivity && drA === drB;
}

type ModSubTab = "expert" | "semantic" | "core" | ExtensionModuleKey;

function renderExpertModPanel(
  levels: {
    expert_level: string;
    expert_level_inherited?: boolean;
    global_expert_level?: string;
  } | null,
  readonly = false,
): string {
  const val = levels?.expert_level || "medium";
  const opts = [
    { value: "low", label: "低", desc: "高置信优先" },
    { value: "medium", label: "中", desc: "推荐默认" },
    { value: "high", label: "高", desc: "更全面覆盖" },
  ];
  const dis = readonly ? " disabled" : "";
  return `
    <div class="module-grid app-config-mod-grid">
      <div class="protection-module-card pl-expert-card">
        <div class="protection-module-head"><strong>专家防护强度</strong></div>
        <div class="protection-module-actions">
          <select class="ant-input pl-level-select" id="app-mod-expert-sel"${dis}>
            ${opts
              .map(
                (o) =>
                  `<option value="${esc(o.value)}" ${o.value === val ? "selected" : ""}>${esc(o.label)} · ${esc(o.desc)}</option>`,
              )
              .join("")}
          </select>
        </div>
      </div>
    </div>`;
}

function collectExpertFromModPanel(root: HTMLElement): {
  expert_level: string;
} | null {
  const sel = root.querySelector("#app-mod-expert-sel") as HTMLSelectElement | null;
  if (!sel) return null;
  return { expert_level: sel.value || "medium" };
}


function renderSemanticModPanel(
  semantic: SemanticConfig,
  defaultSemantic: SemanticConfig,
  siteId: string,
  modSemTab: SemanticTab,
  stats: SemanticStats | null,
  backend: string,
  readonly = false,
): string {
  return `
    ${renderSemanticConsole({
      activeTab: modSemTab,
      cfg: semantic,
      stats,
      backend,
      compact: true,
        toolbarLeft: ``,
      toolbarRight: readonly
        ? ""
        : `<button type="button" class="ant-btn ant-btn-sm" id="app-sem-refresh-stats">刷新观测</button>`,
      footer: "",
      navId: "app-sem-nav",
      panelId: "app-sem-panel",
    })}`;
}

function toModuleCardEntries(
  tuples: [string, ProtectionModules["modules"][string]][],
): ModuleCardEntry[] {
  return tuples.map(([key, cfg]) => ({ key, cfg }));
}

function renderModuleCards(
  entries: ModuleCardEntry[],
  defaultModules: ProtectionModules,
  siteId: string,
  crsStatus: AppCrsStatus | null,
  readonly = false,
): string {
  const isApp = true;
  const crsVer = crsStatus?.binding
    ? (crsStatus.binding.crs_version_label ?? crsStatus.binding.crs_version_id)
    : null;
  const dis = readonly ? " disabled" : "";
  return entries
    .map(({ key, cfg, meta: metaOverride }) => {
      const meta = metaOverride ?? MODULE_META[key] ?? { title: key, desc: "" };
      const crsCat = MODULE_CRS_CATEGORY[key];
      const customSetId = key.startsWith(CUSTOM_MODULE_KEY_PREFIX)
        ? key.slice(CUSTOM_MODULE_KEY_PREFIX.length)
        : "";
      const def = moduleCfgFromRecord(defaultModules.modules, key);
      const inherited = isApp && moduleEquals(cfg, def);
      const disabledCount = cfg.disabled_rules?.length ?? 0;
      const disabledHtml =
        disabledCount > 0
          ? `<span class="mod-disabled-badge muted small" title="${esc((cfg.disabled_rules ?? []).join(", "))}">已禁用 ${disabledCount} 条</span>`
          : "";
      const versionHtml =
        crsCat && crsVer
          ? `<span class="mod-crs-version muted small" title="本应用 CRS 版本">${esc(crsVer)}</span>`
          : crsCat
            ? `<span class="mod-crs-version muted small" title="未绑定版本，跟随系统活跃规则集">未绑定</span>`
            : "";
      const rulesBtn =
        crsCat
          ? `<button type="button" class="ant-btn ant-btn-link ant-btn-sm mod-edit-rules">${readonly ? "查看规则" : "规则集"}</button>`
          : customSetId
            ? `<button type="button" class="ant-btn ant-btn-link ant-btn-sm mod-edit-custom-rules">${readonly ? "查看规则" : "规则集"}</button>`
            : "";
      return `<div class="protection-module-card app-mod-card${cfg.enabled ? "" : " mod-card-disabled"}${readonly ? " is-readonly" : ""}" data-key="${esc(key)}"${crsCat ? ` data-crs-cat="${esc(crsCat)}"` : ""}${customSetId ? ` data-custom-set-id="${esc(customSetId)}" data-custom-set-name="${esc(meta.title)}"` : ""}${inherited ? ` data-inherited="1"` : ""}>
        <div class="protection-module-head">
          <div class="protection-module-title-wrap">
            <strong>${esc(meta.title)}</strong>
            ${versionHtml}
          </div>
          <div class="protection-module-head-actions">
            ${rulesBtn}
            <label class="switch"><input type="checkbox" class="mod-enabled" ${cfg.enabled ? "checked" : ""}${dis}><span></span></label>
          </div>
        </div>
        <p class="muted small">${esc(meta.desc)}${disabledHtml ? ` · ${disabledHtml}` : ""}</p>
        <div class="protection-module-actions">
          <select class="ant-input mod-action"${dis}>
            <option value="block" ${cfg.action === "block" ? "selected" : ""}>拦截</option>
            <option value="log" ${cfg.action === "log" ? "selected" : ""}>仅记录</option>
            <option value="challenge" ${cfg.action === "challenge" ? "selected" : ""}>挑战</option>
          </select>
          <select class="ant-input mod-sensitivity"${dis}>
            <option value="low" ${cfg.sensitivity === "low" ? "selected" : ""}>低</option>
            <option value="medium" ${cfg.sensitivity === "medium" ? "selected" : ""}>中</option>
            <option value="high" ${cfg.sensitivity === "high" ? "selected" : ""}>高</option>
          </select>
        </div>
      </div>`;
    })
    .join("");
}

function renderStrategyPicker(templates: StrategyTemplate[], selectedId: string | null): string {
  const syncIcon = `<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path fill="currentColor" d="M13.65 2.35A7.96 7.96 0 0 0 8 0C3.58 0 0 3.58 0 8h1.5A6.5 6.5 0 0 1 12.6 3.4L10 6h6V0l-2.35 2.35zM2.35 13.65A7.96 7.96 0 0 0 8 16c4.42 0 8-3.58 8-8h-1.5A6.5 6.5 0 0 1 3.4 12.6L6 10H0v6l2.35-2.35z"/></svg>`;
  const chips = templates.length
    ? `<div class="app-strategy-picker-grid">
      ${templates
        .map((t) => {
          const active = t.id === selectedId ? " active" : "";
          const scope = strategyScopeLabel(t);
          const enabled = isStrategyEnabled(t);
          const isApp = isAppPrivateStrategy(t);
          if (!isApp) {
            return `<div class="app-strategy-chip app-strategy-chip--builtin${active}${enabled ? "" : " is-disabled"}" data-strategy-id="${esc(t.id)}">
            <button type="button" class="app-strategy-chip-main" data-strategy-id="${esc(t.id)}"${enabled ? "" : " disabled"}>
              <strong>${esc(t.name)}</strong>
              <span class="muted small">${esc(scope)}${enabled ? "" : " · 已禁用"}</span>
            </button>
            <div class="app-strategy-chip-ops">
              <button type="button" class="app-strategy-chip-sync" title="从策略库同步到本应用" aria-label="从策略库同步到本应用" data-strategy-id="${esc(t.id)}"${enabled ? "" : " disabled"}>${syncIcon}</button>
            </div>
          </div>`;
          }
          return `<div class="app-strategy-chip app-strategy-chip--app${active}${enabled ? "" : " is-disabled"}" data-strategy-id="${esc(t.id)}">
            <button type="button" class="app-strategy-chip-main" data-strategy-id="${esc(t.id)}"${enabled ? "" : " disabled"}>
              <strong>${esc(t.name)}</strong>
              <span class="muted small">${esc(scope)}${enabled ? "" : " · 已禁用"}</span>
            </button>
            <div class="app-strategy-chip-ops">
              ${btn({
                label: "编辑",
                className: "app-strategy-chip-edit",
                size: "sm",
                variant: "link",
                attrs: { "data-strategy-id": t.id },
              })}
              ${btn({
                label: enabled ? "禁用" : "启用",
                className: "app-strategy-chip-toggle",
                size: "sm",
                variant: "link",
                attrs: { "data-strategy-id": t.id, "data-next-enabled": enabled ? "0" : "1" },
              })}
              ${btn({
                label: "删除",
                className: "app-strategy-chip-del",
                size: "sm",
                variant: "link",
                attrs: { "data-strategy-id": t.id },
              })}
            </div>
          </div>`;
        })
        .join("")}
    </div>`
    : `<div class="app-strategy-picker-empty muted">暂无可用策略</div>`;
  return `<div class="app-strategy-picker" role="radiogroup" aria-label="防护策略">
    <div class="app-strategy-picker-head">
      <div class="app-strategy-picker-label">选择防护策略</div>
      ${btn({ id: "app-strategy-new", label: "新建本应用策略", size: "sm" })}
    </div>
    ${chips}
  </div>`;
}

function renderModulesTabBody(
  modules: ProtectionModules,
  defaultModules: ProtectionModules,
  siteId: string,
  modSubTab: ModSubTab,
  modSemTab: SemanticTab,
  customSets: CustomRuleSetInfo[],
  semantic: SemanticConfig,
  defaultSemantic: SemanticConfig,
  semanticStats: SemanticStats | null,
  semanticBackend: string,
  crsStatus: AppCrsStatus | null,
  protectionLevels: {
    expert_level: string;
    expert_level_inherited?: boolean;
    global_expert_level?: string;
  } | null,
  readonly = false,
): string {
  const semanticHtml = renderSemanticModPanel(
    semantic,
    defaultSemantic,
    siteId,
    modSemTab,
    semanticStats,
    semanticBackend,
    readonly,
  );
  const coreHtml = renderModuleCards(
    toModuleCardEntries(orderedWebCoreModuleEntries(modules.modules)),
    defaultModules,
    siteId,
    crsStatus,
    readonly,
  );
  const customEntries = buildCustomModuleEntries(modules.modules, customSets, {
    // Built-in/global read-only: show only rule sets mounted on the strategy snapshot
    onlyMounted: readonly,
  });
  const customHtml =
    customEntries.length > 0
      ? renderModuleCards(customEntries, defaultModules, siteId, crsStatus, readonly)
      : readonly
        ? emptyState("暂无自定义规则集", {
            className: "ant-empty-sm mod-ext-empty",
            description: "可在「防护策略」挂载后，点内置策略卡片上的同步图标写入本应用。",
          })
        : emptyState("暂无规则集", { className: "ant-empty-sm mod-ext-empty" });
  const customPanel = `<div class="mod-sub-panel${modSubTab === "custom" ? "" : " hidden"}" data-mod-sub-panel="custom">
      <div class="module-grid app-config-mod-grid">${customHtml}</div>
    </div>`;

  const inSettings = modSubTab === "semantic";
  const moduleTabs = [
    `<button type="button" class="mod-sub-tab${modSubTab === "expert" ? " active" : ""}" data-mod-sub="expert">专家防护</button>`,
    `<button type="button" class="mod-sub-tab${modSubTab === "core" ? " active" : ""}" data-mod-sub="core">核心防护</button>`,
    `<button type="button" class="mod-sub-tab${modSubTab === "custom" ? " active" : ""}" data-mod-sub="custom">自定义防护</button>`,
  ].join("");

  const modeDis = readonly ? " disabled" : "";
  const isolateHint = readonly
    ? `<p class="app-strategy-isolate-hint muted small">下方为<strong>本应用当前生效</strong>的配置；策略库变更不会自动写入。修改内置策略后，点卡片上的同步图标写入本应用。</p>`
    : "";

  return `
    ${isolateHint}
    <div class="app-config-section-head">
      <label>防护模式
        <select class="ant-input" id="app-mod-mode" style="margin-left:8px"${modeDis}>
          <option value="block" ${modules.mode === "block" ? "selected" : ""}>拦截模式</option>
          <option value="detect" ${modules.mode === "detect" ? "selected" : ""}>检测模式（仅记录）</option>
        </select>
      </label>
    </div>
    <nav class="mod-group-tabs" role="tablist" aria-label="防护分区">
      <button type="button" class="mod-group-tab${inSettings ? " active" : ""}" data-mod-group="settings">防护设置</button>
      <button type="button" class="mod-group-tab${!inSettings ? " active" : ""}" data-mod-group="modules">防护模块</button>
    </nav>
    ${
      inSettings
        ? `<div class="mod-sub-panels">
      <div class="mod-sub-panel" data-mod-sub-panel="semantic">${semanticHtml}</div>
    </div>`
        : `<nav class="mod-sub-tabs mod-sub-tabs--modules">${moduleTabs}</nav>
    <div class="mod-sub-panels">
      <div class="mod-sub-panel${modSubTab === "expert" ? "" : " hidden"}" data-mod-sub-panel="expert">
        ${renderExpertModPanel(protectionLevels, readonly)}
      </div>
      <div class="mod-sub-panel${modSubTab === "core" ? "" : " hidden"}" data-mod-sub-panel="core">
        ${crsStatus && !readonly ? renderAppCrsVersionBar(crsStatus, siteId) : ""}
        <div class="module-grid app-config-mod-grid">${coreHtml}</div>
      </div>
      ${customPanel}
    </div>`
    }`;
}

function bindCustomSetRulesButtons(
  drawer: HTMLElement,
  siteId: string,
  editable: boolean,
): void {
  drawer.querySelectorAll<HTMLButtonElement>(".mod-edit-custom-rules").forEach((btn) => {
    btn.onclick = () => {
      const card = btn.closest<HTMLElement>(".app-mod-card");
      if (!card) return;
      const setId = card.dataset.customSetId;
      const setName = card.dataset.customSetName ?? setId ?? "规则集";
      if (!setId) return;
      const moduleEnabled = (card.querySelector(".mod-enabled") as HTMLInputElement).checked;
      void showCustomSetRulesModal({
        setId,
        setName,
        siteId: editable ? siteId : undefined,
        moduleEnabled,
        editable,
      });
    };
  });
}

function bindModuleRulesButtons(
  drawer: HTMLElement,
  getModules: () => ProtectionModules,
  setModules: (m: ProtectionModules) => void,
  siteId: string,
  editable: boolean,
): void {
  drawer.querySelectorAll<HTMLButtonElement>(".mod-edit-rules").forEach((btn) => {
    btn.onclick = () => {
      const card = btn.closest<HTMLElement>(".app-mod-card")!;
      const key = card.dataset.key!;
      const cat = card.dataset.crsCat;
      if (!cat) return;
      const modMeta = MODULE_META[key] ?? { title: key, desc: "" };
      const sens = (card.querySelector(".mod-sensitivity") as HTMLSelectElement).value;
      const enabled = (card.querySelector(".mod-enabled") as HTMLInputElement).checked;
      const cur = getModules();
      const baseCfg = cur.modules[key];
      void showModuleRulesModal({
        category: cat,
        protect: sensitivityToProtectLevel(sens),
        moduleTitle: modMeta.title,
        moduleEnabled: enabled,
        moduleKey: key,
        siteId,
        editable,
        disabledRules: baseCfg?.disabled_rules ?? [],
        onSave: editable
          ? (disabledRules) => {
              const next = { ...getModules(), modules: { ...getModules().modules } };
              next.modules[key] = {
                ...next.modules[key],
                disabled_rules: disabledRules.length ? disabledRules : undefined,
              };
              setModules(next);
            }
          : undefined,
      });
    };
  });
}

function collectModules(drawer: HTMLElement, base: ProtectionModules): ProtectionModules {
  if (!drawer.querySelector("#app-mod-mode")) return base;
  const mode = (drawer.querySelector("#app-mod-mode") as HTMLSelectElement).value;
  const next: ProtectionModules = { version: base.version, mode, modules: { ...base.modules } };
  drawer.querySelectorAll<HTMLElement>(".app-mod-card[data-key]").forEach((card) => {
    const key = card.dataset.key;
    if (!key) return;
    const enabledEl = card.querySelector(".mod-enabled") as HTMLInputElement | null;
    const actionEl = card.querySelector(".mod-action") as HTMLSelectElement | null;
    const sensEl = card.querySelector(".mod-sensitivity") as HTMLSelectElement | null;
    if (!enabledEl || !actionEl || !sensEl) return;
    const prev = base.modules[key];
    const cfg: ProtectionModules["modules"][string] = {
      enabled: enabledEl.checked,
      action: actionEl.value,
      sensitivity: sensEl.value,
    };
    if (prev?.disabled_rules?.length) cfg.disabled_rules = [...prev.disabled_rules];
    next.modules[key] = cfg;
  });
  return next;
}

function semanticSnapshot(cfg: SemanticConfig): string {
  const order = normalizePipelineOrder(cfg.engines.pipeline_order);
  const enabled = normalizePipelineEnabled(cfg.engines.pipeline_enabled);
  return JSON.stringify({
    core: semanticConfigSnapshot(cfg),
    pipeline_order: order,
    pipeline_enabled: enabled,
    pipeline_editor: {
      mode: cfg.pipeline_editor?.mode ?? "list",
      layout: cfg.pipeline_editor?.layout ?? {},
    },
  });
}

function isListPipelineActive(drawer: HTMLElement): boolean {
  const panel = drawer.querySelector("#pipeline-panel-list");
  return !!panel && !panel.classList.contains("hidden");
}

function collectSemantic(drawer: HTMLElement, base: SemanticConfig): SemanticConfig {
  const q = (id: string) => drawer.querySelector(`#${id}`) as HTMLInputElement | null;
  const orderEl = drawer.querySelector("#app-pipeline-order");
  const semConsole = drawer.querySelector("#app-sem-console");
  const hasSemForm = !!semConsole?.querySelector("#sem-mode");
  const hasPipelineForm = !!orderEl || !!drawer.querySelector("#pipeline-panel-flow");
  if (!hasSemForm && !hasPipelineForm) return base;

  let cfg = hasSemForm ? collectSemanticConfig(semConsole as HTMLElement, base) : { ...base };

  const editorMode = getPipelineEditorMode(drawer);
  const listActive = isListPipelineActive(drawer);
  const listOrder = readPipelineOrder(orderEl as HTMLElement | null);
  const listEnabled = readPipelineEnabled(orderEl as HTMLElement | null);
  const flowState = readPipelineFlowState(drawer);
  const pipelineOrder =
    listActive && listOrder.length
      ? normalizePipelineOrder(listOrder)
      : editorMode === "flow" && flowState.order.length
        ? normalizePipelineOrder(flowState.order)
        : listOrder.length
          ? normalizePipelineOrder(listOrder)
          : hasSemForm && q("eng-order")
            ? normalizePipelineOrder(
                (q("eng-order")!.value || cfg.engines.pipeline_order.join(","))
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean),
              )
            : normalizePipelineOrder(cfg.engines.pipeline_order);
  const pipelineEnabled =
    listActive && orderEl
      ? normalizePipelineEnabled(listEnabled)
      : editorMode === "flow"
        ? normalizePipelineEnabled(flowState.enabled)
        : orderEl
          ? normalizePipelineEnabled(listEnabled)
          : normalizePipelineEnabled(cfg.engines.pipeline_enabled);

  const engines = {
    ...cfg.engines,
    semantic_first: q("eng-sem-first")?.checked ?? cfg.engines.semantic_first,
    libinjection: q("eng-li")?.checked ?? cfg.engines.libinjection,
    sql_intent: q("eng-intent")?.checked ?? cfg.engines.sql_intent,
    fallback_plain_libinjection: q("eng-fallback")?.checked ?? cfg.engines.fallback_plain_libinjection,
    pipeline_order: pipelineOrder,
    pipeline_enabled: pipelineEnabled,
  };
  const rawGraph =
    listActive || editorMode === "list"
      ? rebuildPipelineGraph(pipelineOrder, pipelineEnabled)
      : (flowState.graph ?? cfg.pipeline_editor?.graph ?? rebuildPipelineGraph(pipelineOrder, pipelineEnabled));
  const graph =
    listActive || editorMode === "list" ? rawGraph : (compactPipelineGraph(rawGraph) ?? rawGraph);
  return {
    ...cfg,
    engines,
    intent_enabled: engines.sql_intent,
    pipeline_editor: {
      mode: editorMode,
      layout: { ...cfg.pipeline_editor?.layout, ...flowState.layout },
      graph,
    },
  };
}

function collectSite(drawer: HTMLElement, base: ProtectedSite): ProtectedSite {
  const idEl = drawer.querySelector("#app-id") as HTMLInputElement | null;
  if (!idEl) return base;
  return {
    ...base,
    id: idEl.value.trim(),
    name: (drawer.querySelector("#app-name") as HTMLInputElement).value.trim(),
    hosts: (drawer.querySelector("#app-hosts") as HTMLInputElement).value
      .split(",")
      .map((h) => h.trim())
      .filter(Boolean),
    description: (drawer.querySelector("#app-desc") as HTMLInputElement).value || undefined,
    upstream: (drawer.querySelector("#app-desc") as HTMLInputElement).value || undefined,
    enabled: (drawer.querySelector("#app-enabled") as HTMLInputElement | null)?.checked ?? base.enabled,
  };
}

export type AppConfigDrawerOptions = {
  site: ProtectedSite;
  sites: ProtectedSite[];
  siteIndex: number;
  onSitesChange: (sites: ProtectedSite[]) => void;
  onSaved: () => void;
  toast: (msg: string, ok?: boolean) => void;
};

export async function openAppConfigDrawer(opts: AppConfigDrawerOptions): Promise<void> {
  const { toast, onSaved, onSitesChange } = opts;
  let site = { ...opts.site };
  let sites = [...opts.sites];
  let siteIndex = opts.siteIndex;
  let tab: AppConfigTab = "basic";
  let modSubTab: ModSubTab = "core";
  let modSemTab: SemanticTab = "general";

  const overlay = document.createElement("div");
  overlay.className = "app-config-overlay";
  overlay.innerHTML = `<div class="app-config-drawer"><div class="app-config-body muted" style="padding:48px;text-align:center">加载配置…</div></div>`;
  document.body.appendChild(overlay);

  function close(): void {
    overlay.remove();
  }

  let modules: ProtectionModules;
  let defaultModules: ProtectionModules;
  let customSets: CustomRuleSetInfo[] = [];
  let semantic: SemanticConfig;
  let defaultSemantic: SemanticConfig;
  let initialSiteJson = "";
  let initialModulesJson = "";
  let initialSemanticJson = "";
  let semanticStats: SemanticStats | null = null;
  let semanticBackend = "";
  let crsStatus: AppCrsStatus | null = null;
  let protectionLevels: {
    scope: string;
    semantic_mode: string;
    expert_level: string;
    expert_level_inherited?: boolean;
    global_expert_level?: string;
    global_semantic_mode?: string;
    bound_strategy_id?: string;
    core?: { mode: string; hint: string };
    custom?: { mode: string; hint: string };
  } | null = null;
  let strategyTemplates: StrategyTemplate[] = [];
  let selectedStrategyId: string | null = null;
  let initialBoundStrategyId = "tpl-standard";

  try {
    const [modulesRes, defaultModulesRes, semanticRes, defaultSemanticRes, customRulesRes, crsRes, levelsRes, tplRes] =
      await Promise.all([
      apiGet<ProtectionModules>(`/api/modules${siteQuery(site.id)}`),
      apiGet<ProtectionModules>("/api/global/modules"),
      apiGet<{ config: Partial<SemanticConfig>; backend?: string }>(`/api/semantic/config${siteQuery(site.id)}`).catch(() => ({
        config: undefined,
      })),
      apiGet<{ config: Partial<SemanticConfig> }>("/api/global/semantic/config").catch(() => ({
        config: undefined,
      })),
      apiGet<{ sets: CustomRuleSetInfo[] }>(`/api/custom-rules${siteQuery(site.id)}`).catch(() => ({ sets: [] })),
      fetchAppCrsStatus(site.id).catch(() => null),
      apiGet<NonNullable<typeof protectionLevels>>(`/api/protection-strategy${siteQuery(site.id)}`).catch(
        () =>
          apiGet<NonNullable<typeof protectionLevels>>(`/api/protection-levels${siteQuery(site.id)}`).catch(
            () => null,
          ),
      ),
      fetchStrategyTemplates(site.id),
    ]);
    modules = modulesRes;
    defaultModules = defaultModulesRes;
    customSets = customRulesRes.sets ?? [];
    crsStatus = crsRes;
    protectionLevels = levelsRes;
    strategyTemplates = tplRes;
    selectedStrategyId = levelsRes?.bound_strategy_id ?? "tpl-standard";
    initialBoundStrategyId = selectedStrategyId;
    const boundTpl = strategyTemplates.find((t) => t.id === selectedStrategyId);
    if (!boundTpl || !isStrategyEnabled(boundTpl)) {
      selectedStrategyId = "tpl-standard";
    }
    semantic = mergeSemanticConfig(semanticRes.config);
    defaultSemantic = mergeSemanticConfig(defaultSemanticRes?.config ?? semanticRes.config);
    semanticBackend = (semanticRes as { backend?: string }).backend ?? "";
    initialSiteJson = JSON.stringify(site);
    initialModulesJson = JSON.stringify(modules);
    initialSemanticJson = semanticSnapshot(semantic);
  } catch (e) {
    toast(e instanceof Error ? e.message : String(e), false);
    close();
    return;
  }

  async function refreshCrsStatus(): Promise<void> {
    try {
      crsStatus = await fetchAppCrsStatus(site.id);
    } catch {
      crsStatus = null;
    }
  }

  async function refreshCustomSets(): Promise<void> {
    try {
      const data = await apiGet<{ sets: CustomRuleSetInfo[] }>(`/api/custom-rules${siteQuery(site.id)}`);
      customSets = data.sets ?? [];
    } catch {
      /* Keep the already-loaded list */
    }
  }

  function selectedStrategy(): StrategyTemplate | undefined {
    return strategyTemplates.find((t) => t.id === selectedStrategyId);
  }

  function modulesReadonly(): boolean {
    return !isAppPrivateStrategy(selectedStrategy());
  }

  function applySemanticReadonly(root: HTMLElement): void {
    const consoleEl = root.querySelector("#app-sem-console");
    if (!consoleEl) return;
    consoleEl.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
      "input, select, textarea",
    ).forEach((el) => {
      el.disabled = true;
    });
    consoleEl.querySelectorAll<HTMLButtonElement>("button:not([data-sem-tab])").forEach((btn) => {
      btn.disabled = true;
    });
  }

  function readForm(): void {
    if (tab === "basic") site = collectSite(overlay, site);
    if (tab === "strategy") {
      // Global/built-in strategies are read-only: do not write back from DOM; keep library preview data
      if (!modulesReadonly()) {
        modules = collectModules(overlay, modules);
        if (overlay.querySelector("#app-sem-console")) {
          semantic = collectSemantic(overlay, semantic);
        }
        applyExpertFromModPanel(overlay);
      }
    }
    if (tab === "pipeline") semantic = collectSemantic(overlay, semantic);
  }

  function applyExpertFromModPanel(root: HTMLElement): void {
    if (modulesReadonly()) return;
    const collected = collectExpertFromModPanel(root);
    if (!collected) return;
    semantic = {
      ...semantic,
      expert_level: collected.expert_level as SemanticConfig["expert_level"],
    };
    protectionLevels = {
      ...(protectionLevels ?? {
        scope: "app",
        semantic_mode: semantic.mode,
        expert_level: collected.expert_level,
      }),
      expert_level: collected.expert_level,
      expert_level_inherited: false,
    };
  }

  function syncAllFromDom(): void {
    site = collectSite(overlay, site);
    if (!modulesReadonly()) {
      modules = collectModules(overlay, modules);
      semantic = collectSemantic(overlay, semantic);
      applyExpertFromModPanel(overlay);
    } else if (tab === "pipeline") {
      semantic = collectSemantic(overlay, semantic);
    }
  }

  let strategyPreviewSeq = 0;

  function applyStrategyTemplate(tpl: StrategyTemplate): void {
    selectedStrategyId = tpl.id;
    const tplModules = tpl.core?.modules ?? {};
    modules = {
      version: tpl.core?.version ?? modules.version ?? 1,
      mode: tpl.core?.mode === "detect" ? "detect" : "block",
      modules: Object.fromEntries(
        Object.entries(tplModules).map(([k, v]) => [
          k,
          {
            enabled: !!v.enabled,
            action: v.action || "block",
            sensitivity: v.sensitivity || "medium",
            ...(v.disabled_rules?.length ? { disabled_rules: [...v.disabled_rules] } : {}),
          },
        ]),
      ),
    };
    const mode = (tpl.semantic_mode || "standard") as SemanticConfig["mode"];
    const expert = (tpl.expert_level || "medium") as SemanticConfig["expert_level"];
    semantic = { ...semantic, mode, expert_level: expert };
    protectionLevels = {
      scope: "app",
      semantic_mode: mode,
      expert_level: expert ?? "medium",
      expert_level_inherited: false,
      bound_strategy_id: tpl.id,
    };
  }

  function upsertStrategyCache(tpl: StrategyTemplate): void {
    const idx = strategyTemplates.findIndex((x) => x.id === tpl.id);
    if (idx >= 0) strategyTemplates[idx] = tpl;
    else strategyTemplates = [...strategyTemplates, tpl];
  }

  /** Refresh only strategy-chip selection and the module pane below to avoid full-drawer redraw jank */
  function paintStrategyPanel(): void {
    if (tab !== "strategy") {
      render({ skipReadForm: true });
      return;
    }
    const picker = overlay.querySelector(".app-strategy-picker");
    if (!picker) {
      render({ skipReadForm: true });
      return;
    }
    picker.outerHTML = renderStrategyPicker(strategyTemplates, selectedStrategyId);
    const modulesWrap = overlay.querySelector(".app-strategy-modules");
    if (!modulesWrap) {
      render({ skipReadForm: true });
      return;
    }
    modulesWrap.className = `app-strategy-modules${modulesReadonly() ? " is-readonly" : ""}`;
    modulesWrap.innerHTML = renderModulesTabBody(
      modules,
      defaultModules,
      site.id,
      modSubTab,
      modSemTab,
      customSets,
      semantic,
      defaultSemantic,
      semanticStats,
      semanticBackend,
      crsStatus,
      protectionLevels,
      modulesReadonly(),
    );
    bindStrategyTabInteractions();
  }

  /**
   * Switch strategy chip.
   * Built-in/global: change binding intent only; do not overwrite this app's modules/semantic (isolate library vs runtime).
   * App-private: editable copy; load that strategy snapshot into the form on switch.
   */
  async function selectStrategy(id: string): Promise<void> {
    if (!id) return;
    const seq = ++strategyPreviewSeq;
    const cached = strategyTemplates.find((t) => t.id === id);
    if (cached && !isStrategyEnabled(cached)) {
      toast("该策略已禁用，请先启用或选择其他策略", false);
      return;
    }

    selectedStrategyId = id;

    if (isAppPrivateStrategy(cached)) {
      if (cached) {
        applyStrategyTemplate(cached);
      } else {
        try {
          const tpl = await apiGet<StrategyTemplate>(`/api/protection-strategies/${encodeURIComponent(id)}`);
          if (seq !== strategyPreviewSeq) return;
          if (!isStrategyEnabled(tpl)) {
            upsertStrategyCache(tpl);
            toast("该策略已禁用，请先启用或选择其他策略", false);
            return;
          }
          upsertStrategyCache(tpl);
          applyStrategyTemplate(tpl);
        } catch {
          if (seq === strategyPreviewSeq) toast("加载策略快照失败", false);
        }
      }
    }

    paintStrategyPanel();
  }

  /** After opening the drawer, refresh strategy-library cache in background (do not write app runtime) */
  async function refreshStrategyCacheInBackground(id: string): Promise<void> {
    try {
      const tpl = await apiGet<StrategyTemplate>(`/api/protection-strategies/${encodeURIComponent(id)}`);
      upsertStrategyCache(tpl);
      if (selectedStrategyId === id && tab === "strategy") {
        paintStrategyPanel();
      }
    } catch {
      /* Silent */
    }
  }

  /** @deprecated kept for post-create callers */
  async function previewStrategyLocally(id: string): Promise<void> {
    await selectStrategy(id);
  }

  function bindStrategyTabInteractions(): void {
    if (modulesReadonly()) {
      applySemanticReadonly(overlay);
    }
    overlay
      .querySelectorAll<HTMLButtonElement>(".app-strategy-chip-main")
      .forEach((chip) => {
      chip.addEventListener("click", () => {
        const id = chip.dataset.strategyId;
        if (!id || id === selectedStrategyId) return;
        try {
          readForm();
        } catch {
          /* keep */
        }
        void selectStrategy(id);
      });
    });
    overlay.querySelectorAll<HTMLButtonElement>(".app-strategy-chip-sync").forEach((node) => {
      node.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const id = node.dataset.strategyId;
        if (!id) return;
        void (async () => {
          const tpl =
            strategyTemplates.find((t) => t.id === id) ??
            (await apiGet<StrategyTemplate>(`/api/protection-strategies/${encodeURIComponent(id)}`).catch(
              () => null,
            ));
          if (!tpl?.builtin) {
            await showResult({
              type: "info",
              title: "无法同步",
              message: "仅内置策略支持从策略库同步。",
            });
            return;
          }
          if (!isStrategyEnabled(tpl)) {
            await showResult({
              type: "info",
              title: "无法同步",
              message: "该内置策略已禁用。",
            });
            return;
          }
          const name = tpl.name || id;
          const ok = await runMutatingAction({
            action: "save",
            subject: `内置策略「${name}」`,
            confirmMessage:
              "将用策略库最新配置覆盖本应用当前防护设置？此操作会立即写入并生效。",
            successMessage: `已从策略库同步「${name}」到本应用`,
            run: async () => {
              const latest = await apiGet<StrategyTemplate>(
                `/api/protection-strategies/${encodeURIComponent(id)}`,
              );
              if (!latest?.builtin) throw new Error("策略不是内置模板，无法同步");
              if (!isStrategyEnabled(latest)) throw new Error("该内置策略已禁用");
              await apiPost(`/api/protection-strategies/${encodeURIComponent(latest.id)}/apply`, {
                target: "app",
                site_id: site.id,
              });
              const [modulesRes, semanticRes, levelsRes] = await Promise.all([
                apiGet<ProtectionModules>(`/api/modules${siteQuery(site.id)}`),
                apiGet<{ config: Partial<SemanticConfig> }>(`/api/semantic/config${siteQuery(site.id)}`),
                apiGet<NonNullable<typeof protectionLevels>>(
                  `/api/protection-strategy${siteQuery(site.id)}`,
                ).catch(() => null),
              ]);
              modules = modulesRes;
              semantic = mergeSemanticConfig(semanticRes.config);
              if (levelsRes) protectionLevels = levelsRes;
              upsertStrategyCache(latest);
              selectedStrategyId = latest.id;
              initialModulesJson = JSON.stringify(modules);
              initialSemanticJson = semanticSnapshot(semantic);
              initialBoundStrategyId = latest.id;
              if (site.id === "default") {
                void apiPost("/api/sync").catch(() => null);
              }
              onSaved();
            },
          });
          if (ok) paintStrategyPanel();
        })();
      });
    });
    overlay.querySelectorAll<HTMLButtonElement>(".app-strategy-chip-edit").forEach((node) => {
      node.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const id = node.dataset.strategyId;
        if (!id) return;
        const tpl = strategyTemplates.find((t) => t.id === id);
        void (async () => {
          const values = await showPrompt({
            title: "编辑本应用策略",
            okText: "保存",
            cancelText: "取消",
            fields: [
              { id: "name", label: "名称", defaultValue: tpl?.name ?? "", wide: true },
              {
                id: "description",
                label: "说明",
                defaultValue: tpl?.description ?? "",
                placeholder: "可选",
                wide: true,
              },
            ],
          });
          if (!values) return;
          const name = values.name?.trim();
          if (!name) {
            await showResult({ type: "error", title: "校验失败", message: "请填写名称" });
            return;
          }
          const ok = await runMutatingAction({
            action: "edit",
            subject: `策略「${name}」`,
            confirmMessage: `保存「${name}」的名称与说明？`,
            successMessage: "已保存",
            run: async () => {
              const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
                `/api/protection-strategies/${encodeURIComponent(id)}`,
                { name, description: values.description?.trim() || "" },
              );
              if (!resp.template) throw new Error("未返回策略");
              upsertStrategyCache(resp.template);
            },
          });
          if (ok) paintStrategyPanel();
        })();
      });
    });
    overlay.querySelectorAll<HTMLButtonElement>(".app-strategy-chip-toggle").forEach((node) => {
      node.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const id = node.dataset.strategyId;
        if (!id) return;
        const next = node.dataset.nextEnabled === "1";
        const tpl = strategyTemplates.find((t) => t.id === id);
        void (async () => {
          const ok = await runMutatingAction({
            action: "edit",
            subject: `策略「${tpl?.name ?? id}」`,
            confirmMessage: next
              ? `启用「${tpl?.name ?? id}」？`
              : `禁用「${tpl?.name ?? id}」？禁用后不可再选绑；若当前绑定该策略将回退到「标准」。`,
            successMessage: next ? "已启用" : "已禁用",
            run: async () => {
              const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
                `/api/protection-strategies/${encodeURIComponent(id)}`,
                { enabled: next },
              );
              if (!resp.template) throw new Error("未返回策略");
              upsertStrategyCache(resp.template);
              if (!next && selectedStrategyId === id) {
                if (initialBoundStrategyId === id) initialBoundStrategyId = "tpl-standard";
                selectedStrategyId = "tpl-standard";
              }
            },
          });
          if (ok) paintStrategyPanel();
        })();
      });
    });
    overlay.querySelectorAll<HTMLButtonElement>(".app-strategy-chip-del").forEach((node) => {
      node.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const id = node.dataset.strategyId;
        if (!id) return;
        const tpl = strategyTemplates.find((t) => t.id === id);
        void (async () => {
          const ok = await runMutatingAction({
            action: "delete",
            subject: `策略「${tpl?.name ?? id}」`,
            confirmMessage: `确定删除「${tpl?.name ?? id}」？删除后不可恢复；若当前绑定该策略将回退到「标准」。`,
            successMessage: "已删除",
            run: async () => {
              await apiDelete(`/api/protection-strategies/${encodeURIComponent(id)}`);
              strategyTemplates = strategyTemplates.filter((t) => t.id !== id);
              if (selectedStrategyId === id) {
                if (initialBoundStrategyId === id) initialBoundStrategyId = "tpl-standard";
                selectedStrategyId = "tpl-standard";
              }
            },
          });
          if (ok) paintStrategyPanel();
        })();
      });
    });
    overlay.querySelector("#app-strategy-new")?.addEventListener("click", () => {
      void (async () => {
        const values = await showPrompt({
          title: "新建本应用策略",
          okText: "创建",
          cancelText: "取消",
          fields: [
            { id: "name", label: "名称", defaultValue: `${site.name}-策略`, wide: true },
            { id: "description", label: "说明", defaultValue: "", placeholder: "可选", wide: true },
          ],
        });
        if (!values) return;
        const name = values.name?.trim();
        if (!name) {
          await showResult({ type: "error", title: "校验失败", message: "请填写名称" });
          return;
        }
        const ok = await runMutatingAction({
          action: "create",
          subject: `本应用策略「${name}」`,
          confirmMessage: `创建「${name}」？`,
          successMessage: `「${name}」已创建`,
          run: async () => {
            const resp = await apiPost<{ ok: boolean; template: StrategyTemplate }>(
              "/api/protection-strategies",
              {
                name,
                description: values.description?.trim() || "",
                site_id: site.id,
                source_id: selectedStrategyId || undefined,
              },
            );
            if (!resp.template) throw new Error("未返回策略");
            if (!isAppPrivateStrategy(resp.template)) {
              throw new Error("创建失败：未写入本应用策略，请重启 API 后重试");
            }
            upsertStrategyCache(resp.template);
            applyStrategyTemplate(resp.template);
          },
        });
        if (ok) paintStrategyPanel();
      })();
    });
    if (!modulesReadonly()) {
      bindModuleCardStyles(overlay);
    }
    {
      const canEditModules = !modulesReadonly();
      bindCustomSetRulesButtons(overlay, site.id, canEditModules);
      bindModuleRulesButtons(
        overlay,
        () => modules,
        (m) => {
          modules = m;
        },
        site.id,
        canEditModules,
      );
    }
    overlay.querySelectorAll<HTMLButtonElement>(".mod-group-tab").forEach((btn) => {
      btn.addEventListener("click", () => {
        void (async () => {
          try {
            readForm();
          } catch (e) {
            console.warn("app-config readForm", e);
          }
          const group = btn.dataset.modGroup;
          if (group === "settings") {
            modSubTab = "semantic";
            if (!semanticStats) {
              semanticStats = await apiGet<SemanticStats>(`/api/semantic/stats${siteQuery(site.id)}`).catch(
                () => null,
              );
            }
          } else if (group === "modules" && modSubTab === "semantic") {
            modSubTab = "core";
          }
          paintStrategyPanel();
        })();
      });
    });
    overlay.querySelectorAll<HTMLButtonElement>(".mod-sub-tab").forEach((btn) => {
      btn.addEventListener("click", () => {
        void (async () => {
          try {
            readForm();
          } catch (e) {
            console.warn("app-config readForm", e);
          }
          modSubTab = (btn.dataset.modSub as ModSubTab) ?? "core";
          if (modSubTab === "custom") await refreshCustomSets();
          paintStrategyPanel();
        })();
      });
    });
    bindSemanticModPanel();
    if (!modulesReadonly()) {
      bindExpertModPanel();
      bindAppCrsVersionBar(overlay, () => {
        void (async () => {
          await refreshCrsStatus();
          try {
            modules = await apiGet<ProtectionModules>(`/api/modules${siteQuery(site.id)}`);
          } catch {
            /* keep */
          }
          paintStrategyPanel();
        })();
      });
    }
  }

  function bindExpertModPanel(): void {
    /* This app's expert tier is independent; no global inheritance */
  }

  function bindSemanticModPanel(): void {
    const consoleEl = overlay.querySelector("#app-sem-console");
    if (!consoleEl) return;
    const locked = modulesReadonly();

    const syncSem = () => {
      if (locked) return;
      semantic = collectSemantic(overlay, semantic);
    };

    if (!locked) {
      bindSemanticDecodeOrder(consoleEl as HTMLElement, syncSem);
    }

    consoleEl.querySelectorAll<HTMLButtonElement>("[data-sem-tab]").forEach((btn) => {
      btn.onclick = () => {
        syncSem();
        modSemTab = (btn.getAttribute("data-sem-tab") as SemanticTab) || "general";
        render();
      };
    });

    if (locked) return;

    overlay.querySelector("#app-sem-refresh-stats")?.addEventListener("click", () => {
      void (async () => {
        try {
          semanticStats = await apiGet<SemanticStats>(`/api/semantic/stats${siteQuery(site.id)}`);
          modSemTab = "observe";
          render();
          toast("观测数据已刷新");
        } catch (e) {
          toast(String(e), false);
        }
      })();
    });

    overlay.querySelector("#btn-fb")?.addEventListener("click", () => {
      void (async () => {
        try {
          const resp = await apiPost<{ config_updated?: boolean; auto_tune?: { actions?: string[] } }>(
            "/api/semantic/feedback",
            {
              site_id: site.id,
              event_id: (overlay.querySelector("#fb-event") as HTMLInputElement).value || undefined,
              verdict: (overlay.querySelector("#fb-verdict") as HTMLSelectElement).value,
              note: (overlay.querySelector("#fb-note") as HTMLInputElement).value || undefined,
            },
          );
          semanticStats = await apiGet<SemanticStats>(`/api/semantic/stats${siteQuery(site.id)}`).catch(
            () => semanticStats,
          );
          if (resp.config_updated) {
            const cfgRes = await apiGet<{ config: Partial<SemanticConfig> }>(
              `/api/semantic/config${siteQuery(site.id)}`,
            );
            semantic = mergeSemanticConfig(cfgRes.config);
          }
          toast(
            resp.config_updated
              ? `反馈已记录，已自动调优: ${resp.auto_tune?.actions?.join(", ") ?? ""}`
              : "反馈已记录",
          );
          render();
        } catch (e) {
          toast(String(e), false);
        }
      })();
    });
  }

  function bindPipelineIfPresent(): void {
    if (overlay.querySelector("#app-pipeline-order")) {
      bindPipelineOrder(overlay, () => {
        semantic = collectSemantic(overlay, semantic);
      });
    }
    if (overlay.querySelector("#pipeline-panel-flow")) {
      bindPipelineFlowEditor(
        overlay,
        normalizePipelineOrder(semantic.engines.pipeline_order),
        mergePipelineEditor(semantic.pipeline_editor),
        normalizePipelineEnabled(semantic.engines.pipeline_enabled),
        () => {
          semantic = collectSemantic(overlay, semantic);
        },
      );
    }
    bindPipelineModeSwitch(overlay, (_oldMode, newMode) => {
      semantic = collectSemantic(overlay, semantic);
      semantic.pipeline_editor = { ...semantic.pipeline_editor!, mode: newMode };
      const order = semantic.engines.pipeline_order;
      const enabled = normalizePipelineEnabled(semantic.engines.pipeline_enabled);
      const editor = mergePipelineEditor(semantic.pipeline_editor);
      if (newMode === "flow") {
        destroyPipelineFlowEditor(overlay);
        const flowPanel = overlay.querySelector("#pipeline-panel-flow");
        if (flowPanel) {
          flowPanel.innerHTML = renderPipelineFlowEditor(order, editor, esc);
          bindPipelineFlowEditor(overlay, order, editor, enabled, () => {
            semantic = collectSemantic(overlay, semantic);
          });
        }
      } else {
        destroyPipelineFlowEditor(overlay);
        const list = overlay.querySelector("#app-pipeline-order");
        if (list) {
          list.innerHTML = renderPipelineOrderList(order, enabled, esc);
          bindPipelineOrder(overlay, () => {
            semantic = collectSemantic(overlay, semantic);
          });
        }
      }
    });
  }

  function render(opts?: { skipReadForm?: boolean }): void {
    if (!opts?.skipReadForm && overlay.querySelector(".app-config-tabs")) {
      readForm();
    }
    const pipelineOrder = normalizePipelineOrder(semantic.engines.pipeline_order);
    const pipelineEnabled = normalizePipelineEnabled(semantic.engines.pipeline_enabled);
    const editor = mergePipelineEditor(semantic.pipeline_editor);
    const editorMode = editor.mode;

    const tabBody =
      tab === "basic"
        ? `
        <div class="form-stack" style="max-width:560px">
          <label class="ant-form-label">应用 ID</label>
          <input class="ant-input ant-input-wide" id="app-id" value="${esc(site.id)}" ${site.id === "default" ? "readonly" : ""} />
          <label class="ant-form-label">名称</label>
          <input class="ant-input ant-input-wide" id="app-name" value="${esc(site.name)}" />
          <label class="ant-form-label">Host（逗号分隔）</label>
          <input class="ant-input ant-input-wide" id="app-hosts" value="${esc(site.hosts.join(","))}" placeholder="api.example.com" />
          <label class="ant-form-label">备注</label>
          <input class="ant-input ant-input-wide" id="app-desc" value="${esc(site.description ?? site.upstream ?? "")}" placeholder="上游 URL 或说明" />
          ${switchHtml("app-enabled", site.enabled, "启用防护")}
        </div>`
        : tab === "strategy"
            ? `${renderStrategyPicker(strategyTemplates, selectedStrategyId)}
            <div class="app-strategy-modules${modulesReadonly() ? " is-readonly" : ""}">
              ${renderModulesTabBody(
                modules,
                defaultModules,
                site.id,
                modSubTab,
                modSemTab,
                customSets,
                semantic,
                defaultSemantic,
                semanticStats,
                semanticBackend,
                crsStatus,
                protectionLevels,
                modulesReadonly(),
              )}
            </div>`
            : `
        <div class="form-stack pipeline-tab-stack">
          <div class="pipeline-editor-section">
            <div class="pipeline-editor-head">
              <div>
                <label class="ant-form-label" style="margin:0">检测流水线</label>
              </div>
              <div class="pipeline-mode-switch">
                <button type="button" class="pipeline-mode-btn ${editorMode === "list" ? "active" : ""}" data-mode="list">列表模式</button>
                <button type="button" class="pipeline-mode-btn ${editorMode === "flow" ? "active" : ""}" data-mode="flow">编排模式</button>
              </div>
            </div>
            <input type="hidden" id="app-pipeline-editor-mode" value="${editorMode}" />
            <div id="pipeline-panel-list" class="pipeline-panel ${editorMode === "list" ? "" : "hidden"}">
              <div id="app-pipeline-order" class="pipeline-order-list">${renderPipelineOrderList(pipelineOrder, pipelineEnabled, esc)}</div>
            </div>
            <div id="pipeline-panel-flow" class="pipeline-panel ${editorMode === "flow" ? "" : "hidden"}">
              ${renderPipelineFlowEditor(pipelineOrder, editor, esc)}
            </div>
          </div>
        </div>`;

    overlay.innerHTML = `
      <div class="app-config-drawer">
        <div class="app-config-head">
          <div>
            <h3>应用配置</h3>
            <p class="muted small">${esc(site.name)} · <code>${esc(site.id)}</code></p>
          </div>
          <button type="button" class="ant-btn ant-btn-link" id="app-close">✕</button>
        </div>
        <nav class="app-config-tabs">
          <button type="button" class="app-config-tab ${tab === "basic" ? "active" : ""}" data-tab="basic">① 基础信息</button>
          <button type="button" class="app-config-tab ${tab === "strategy" ? "active" : ""}" data-tab="strategy">② 防护策略</button>
          <button type="button" class="app-config-tab ${tab === "pipeline" ? "active" : ""}" data-tab="pipeline">③ 检测流水线</button>
        </nav>
        <div class="app-config-body">${tabBody}</div>
        <div class="app-config-foot">
          <span></span>
          <div>
            <button type="button" class="ant-btn" id="app-cancel">取消</button>
            <button type="button" class="ant-btn ant-btn-primary" id="app-save-all">保存并生效</button>
          </div>
        </div>
      </div>`;

    overlay.querySelector("#app-close")?.addEventListener("click", close);
    overlay.querySelector("#app-cancel")?.addEventListener("click", close);
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) close();
    });

    overlay.querySelectorAll(".app-config-tab").forEach((btn) => {
      btn.addEventListener("click", () => {
        void (async () => {
          try {
            readForm();
          } catch (e) {
            console.warn("app-config readForm", e);
          }
          tab = (btn as HTMLElement).dataset.tab as AppConfigTab;
          if (tab === "strategy") await refreshCustomSets();
          render();
        })();
      });
    });

    bindPipelineIfPresent();
    if (tab === "strategy") {
      bindStrategyTabInteractions();
    }

    overlay.querySelector("#app-save-all")?.addEventListener("click", () => {
      void (async () => {
        syncAllFromDom();
        const idErr = validateSiteId(site, sites, siteIndex);
        if (idErr) {
          await showResult({ type: "error", title: "校验失败", message: idErr });
          return;
        }
        const siteDirty = JSON.stringify(site) !== initialSiteJson;
        let modulesDirty = JSON.stringify(modules) !== initialModulesJson;
        let semanticDirty = semanticSnapshot(semantic) !== initialSemanticJson;
        const bindingDirty = (selectedStrategyId ?? "") !== initialBoundStrategyId;
        const privateTpl = !modulesReadonly() && !!selectedStrategyId;
        // App-private strategy: module changes must write back to the library or the next selection loses them
        if (privateTpl && (modulesDirty || semanticDirty || bindingDirty)) {
          modulesDirty = true;
        }
        if (!siteDirty && !modulesDirty && !semanticDirty && !bindingDirty) {
          await showResult({ type: "info", title: "无需保存", message: "没有需要保存的变更" });
          return;
        }

        const ok = await runMutatingAction({
            action: "save",
            subject: `应用「${site.name}」配置`,
            confirmMessage: "确定保存并生效吗？",
            successMessage: `应用「${site.name}」已保存`,
            run: async () => {
              if (siteDirty) {
                const oldId = sites[siteIndex]?.id ?? site.id;
                if (site.id !== oldId && siteIndex >= 0) {
                  sites[siteIndex] = site;
                } else if (siteIndex >= 0) {
                  sites[siteIndex] = site;
                } else {
                  sites.push(site);
                  siteIndex = sites.length - 1;
                }
                onSitesChange(sites);
              }

              const errors: string[] = [];
              if (siteDirty) {
                try {
                  await apiPut("/api/sites", sites);
                  initialSiteJson = JSON.stringify(site);
                } catch (e) {
                  errors.push(`基础信息：${e instanceof Error ? e.message : String(e)}`);
                }
              }
              // App-private strategy: write back the four strategy sections first, then app runtime
              if (privateTpl && selectedStrategyId && (modulesDirty || semanticDirty)) {
                try {
                  const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
                    `/api/protection-strategies/${encodeURIComponent(selectedStrategyId)}`,
                    {
                      core: modules,
                      semantic_mode: semantic.mode,
                      expert_level: semantic.expert_level ?? "medium",
                      expert: { level: semantic.expert_level ?? "medium" },
                    },
                  );
                  if (resp.template) {
                    const idx = strategyTemplates.findIndex((x) => x.id === resp.template.id);
                    if (idx >= 0) strategyTemplates[idx] = resp.template;
                  }
                } catch (e) {
                  errors.push(`本应用策略：${e instanceof Error ? e.message : String(e)}`);
                }
              }
              if (modulesDirty) {
                try {
                  await apiPut(`/api/modules${siteQuery(site.id)}`, modules);
                  initialModulesJson = JSON.stringify(modules);
                } catch (e) {
                  errors.push(`防护模块：${e instanceof Error ? e.message : String(e)}`);
                }
              }
              if (semanticDirty) {
                try {
                  await apiPut(`/api/semantic/config${siteQuery(site.id)}`, semantic, 20_000);
                  protectionLevels = {
                    scope: "app",
                    semantic_mode: semantic.mode,
                    expert_level: semantic.expert_level ?? "medium",
                    expert_level_inherited: false,
                    bound_strategy_id: selectedStrategyId ?? undefined,
                  };
                  initialSemanticJson = semanticSnapshot(semantic);
                } catch (e) {
                  errors.push(`检测流水线：${e instanceof Error ? e.message : String(e)}`);
                }
              }
              if ((bindingDirty || privateTpl || modulesDirty || semanticDirty) && selectedStrategyId) {
                try {
                  await apiPut(`/api/protection-strategy${siteQuery(site.id)}`, {
                    bound_strategy_id: selectedStrategyId,
                  });
                  initialBoundStrategyId = selectedStrategyId;
                  protectionLevels = {
                    ...(protectionLevels ?? {
                      scope: "app",
                      semantic_mode: semantic.mode,
                      expert_level: semantic.expert_level ?? "medium",
                    }),
                    bound_strategy_id: selectedStrategyId,
                  };
                } catch (e) {
                  errors.push(`策略绑定：${e instanceof Error ? e.message : String(e)}`);
                }
              }
              if (errors.length > 0) throw new Error(errors.join("；"));

              if (site.id === "default" && (modulesDirty || semanticDirty)) {
                void apiPost("/api/sync").catch(() => null);
              }
              // Stay in the current drawer after save; do not close
              onSaved();
            },
          });
        if (!ok) return;
      })();
    });
  }

  render({ skipReadForm: true });
  if (selectedStrategyId) {
    void refreshStrategyCacheInBackground(selectedStrategyId);
  }
}
