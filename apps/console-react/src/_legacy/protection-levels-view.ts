/** Legacy (pre-React) console module: Protection levels view. */
import { apiDelete, apiGet, apiPost, apiPut } from "./api.js";
import {
  btn,
  el,
  esc,
  hideModal,
  runMutatingAction,
  showPrompt,
  showResult,
  tag,
} from "./components/ui.js";
import {
  CUSTOM_MODULE_KEY_PREFIX,
  MODULE_META,
  customModuleKey,
  orderedWebCoreModuleEntries,
  type CustomRuleSetInfo,
} from "./module-meta.js";
import { bindModuleCardStyles } from "./module-display.js";
import { loadSites } from "./sites.js";

/** Currently effective strategy (global default / app snapshot) */
export type ProtectionStrategy = {
  scope: string;
  name?: string;
  semantic_mode: string;
  expert_level: string;
  expert_level_inherited?: boolean;
  global_expert_level?: string;
  global_semantic_mode?: string;
  core?: { mode: string; hint: string };
  custom?: { mode: string; hint: string };
};

/** Named template (strategy library): includes four-section snapshot */
export type StrategyTemplate = {
  id: string;
  name: string;
  description: string;
  semantic_mode: string;
  expert_level: string;
  builtin: boolean;
  sort_order: number;
  /** Set = app-private strategy; omitted = global */
  site_id?: string | null;
  scope?: "global" | "app";
  /** App-private strategy may be disabled; default true */
  enabled?: boolean;
  expert?: { level?: string };
  semantic?: Record<string, unknown>;
  core?: {
    version?: number;
    mode?: string;
    modules?: Record<
      string,
      { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }
    >;
  };
  custom?: Record<string, unknown>;
};

export function strategyScopeLabel(t: StrategyTemplate): string {
  if (t.site_id || t.scope === "app") return "本应用";
  if (t.builtin) return "内置";
  return "全局";
}

/** Only app-private (custom) strategies can edit modules below; global/built-in are read-only library previews */
export function isAppPrivateStrategy(t: StrategyTemplate | null | undefined): boolean {
  return !!(t?.site_id && String(t.site_id).trim());
}

export function isStrategyEnabled(t: StrategyTemplate | null | undefined): boolean {
  return t?.enabled !== false;
}

const SEMANTIC_OPTS = [
  { value: "relaxed", label: "宽松", desc: "更少误报" },
  { value: "standard", label: "标准", desc: "推荐默认" },
  { value: "strict", label: "严格", desc: "更严拦截" },
] as const;

const EXPERT_OPTS = [
  { value: "low", label: "低", desc: "高置信优先" },
  { value: "medium", label: "中", desc: "推荐默认" },
  { value: "high", label: "高", desc: "更全面覆盖" },
] as const;

function modeLabel(v: string): string {
  return SEMANTIC_OPTS.find((o) => o.value === v)?.label ?? v;
}

function levelLabel(v: string): string {
  return EXPERT_OPTS.find((o) => o.value === v)?.label ?? v;
}

function tierClass(name: string): string {
  if (name.includes("宽松")) return "pl-tier-relaxed";
  if (name.includes("严格")) return "pl-tier-strict";
  return "pl-tier-standard";
}

function toneForMode(mode: string): "green" | "blue" | "orange" | "default" {
  if (mode === "relaxed") return "green";
  if (mode === "strict") return "orange";
  if (mode === "standard") return "blue";
  return "default";
}

function optionCards(
  name: string,
  options: readonly { value: string; label: string; desc: string }[],
  selected: string,
): string {
  return `<div class="pl-option-group" role="radiogroup">
    ${options
      .map(
        (o) => `<label class="pl-option${o.value === selected ? " pl-option-active" : ""}">
        <input type="radio" name="${esc(name)}" value="${esc(o.value)}" ${o.value === selected ? "checked" : ""} />
        <span class="pl-option-label">${esc(o.label)}</span>
        <span class="pl-option-desc">${esc(o.desc)}</span>
      </label>`,
      )
      .join("")}
  </div>`;
}

function levelSelect(
  id: string,
  options: readonly { value: string; label: string }[],
  selected: string,
): string {
  return `<select class="ant-input pl-level-select" id="${esc(id)}">
    ${options
      .map(
        (o) =>
          `<option value="${esc(o.value)}" ${o.value === selected ? "selected" : ""}>${esc(o.label)}</option>`,
      )
      .join("")}
  </select>`;
}

function levelCard(opts: {
  title: string;
  desc: string;
  controlHtml: string;
  extraHtml?: string;
}): string {
  return `<div class="protection-module-card pl-stage-card">
    <div class="protection-module-head"><strong>${esc(opts.title)}</strong></div>
    ${opts.desc ? `<p class="muted">${esc(opts.desc)}</p>` : ""}
    <div class="protection-module-actions">${opts.controlHtml}</div>
    ${opts.extraHtml ?? ""}
  </div>`;
}

function renderStrategyGrid(data: ProtectionStrategy, opts?: { app?: boolean }): string {
  const app = opts?.app === true;
  const semanticId = app ? "app-pl-semantic-sel" : "pl-semantic-sel";
  const expertId = app ? "app-pl-expert-sel" : "pl-expert-sel";
  const semanticVal = data.semantic_mode || "standard";
  const expertVal = data.expert_level || "medium";

  const semanticCard = levelCard({
    title: "语义设置",
    desc: "",
    controlHtml: levelSelect(semanticId, SEMANTIC_OPTS, semanticVal),
  });

  const expertCard = levelCard({
    title: "专家防护",
    desc: "",
    controlHtml: levelSelect(expertId, EXPERT_OPTS, expertVal),
  });

  const coreCard = levelCard({
    title: "核心防护",
    desc: "",
    controlHtml: app
      ? `<span class="muted small">防护模块</span>`
      : `<a class="ant-btn ant-btn-sm" href="#/modules">核心防护</a>`,
  });

  const customCard = levelCard({
    title: "自定义检测",
    desc: "",
    controlHtml: app
      ? `<span class="muted small">自定义规则</span>`
      : `<a class="ant-btn ant-btn-sm" href="#/rules-custom">签名规则</a>`,
  });

  return `<div class="module-grid pl-levels-grid">${semanticCard}${expertCard}${coreCard}${customCard}</div>`;
}

function renderTemplateCards(templates: StrategyTemplate[], selectedId: string | null): string {
  if (!templates.length) {
    return `<div class="pl-empty"><span class="muted">暂无策略</span>${btn({ id: "pl-tpl-new-empty", label: "新建策略", variant: "primary", size: "sm" })}</div>`;
  }
  return `<div class="pl-strategy-grid">
    ${templates
      .map((t) => {
        const active = t.id === selectedId ? " pl-tpl-active" : "";
        const tier = tierClass(t.name);
        return `<div class="pl-strategy-card ${tier}${active}" data-tpl-id="${esc(t.id)}">
          <div class="pl-strategy-card-accent"></div>
          <div class="pl-strategy-card-body">
            <div class="pl-strategy-card-top">
              <h3 class="pl-strategy-name">${esc(t.name)}</h3>
              ${t.builtin ? tag("内置", "blue") : tag(strategyScopeLabel(t), t.site_id ? "green" : "default")}
            </div>
            <dl class="pl-strategy-kv">
              <div><dt>语义</dt><dd>${tag(modeLabel(t.semantic_mode), toneForMode(t.semantic_mode))}</dd></div>
              <div><dt>专家</dt><dd>${tag(levelLabel(t.expert_level), "default")}</dd></div>
            </dl>
            <div class="pl-strategy-card-foot">
              ${btn({ label: "编辑", className: "pl-tpl-edit", variant: "primary", attrs: { "data-tpl-id": t.id } })}
              ${btn({
                label: "套用到应用",
                className: "pl-tpl-apply",
                attrs: { "data-tpl-id": t.id },
              })}
              ${
                !t.builtin && !t.site_id
                  ? btn({
                      label: "设为全局",
                      className: "pl-tpl-apply-global",
                      attrs: { "data-tpl-id": t.id },
                    })
                  : ""
              }
              ${
                t.builtin
                  ? ""
                  : btn({
                      label: "删除",
                      className: "pl-tpl-del",
                      attrs: { "data-tpl-id": t.id },
                    })
              }
            </div>
          </div>
        </div>`;
      })
      .join("")}
  </div>`;
}

function renderStrategyModuleEditor(tpl: StrategyTemplate): string {
  // Protection settings (semantic) + modules (expert / core / custom)
  const settings = [
    {
      key: "semantic",
      title: "语义设置",
      icon: "语",
      value: modeLabel(tpl.semantic_mode),
      tone: toneForMode(tpl.semantic_mode),
    },
  ];
  const modules = [
    {
      key: "expert",
      title: "专家防护",
      icon: "专",
      value: levelLabel(tpl.expert_level),
      tone: "blue" as const,
    },
    { key: "core", title: "核心防护", icon: "核", value: "按模块", tone: "orange" as const },
    { key: "custom", title: "自定义防护", icon: "自", value: (() => {
      const n = strategyCustomSetIds(tpl).length;
      return n ? `${n} 个规则集` : "未挂载";
    })(), tone: "default" as const },
  ];
  const row = (s: (typeof settings)[0]) =>
    `<button type="button" class="pl-stage-row" data-stage="${esc(s.key)}">
      <span class="pl-stage-icon">${esc(s.icon)}</span>
      <span class="pl-stage-main"><strong>${esc(s.title)}</strong></span>
      <span class="pl-stage-value">${tag(s.value, s.tone)}</span>
      <span class="pl-stage-chevron">›</span>
    </button>`;
  return `
    <div class="pl-edit-page" id="pl-tpl-editor">
      <div class="pl-edit-bar">
        ${btn({ id: "pl-tpl-back", label: "返回", size: "sm" })}
        <div class="pl-edit-title">
          <h2>${esc(tpl.name)}</h2>
          ${tpl.builtin ? tag("内置", "blue") : tag("自定义", "default")}
        </div>
      </div>
      <p class="pl-section-label">防护设置</p>
      <div class="pl-stage-list">${settings.map(row).join("")}</div>
      <p class="pl-section-label">防护模块</p>
      <div class="pl-stage-list">${modules.map(row).join("")}</div>
    </div>`;
}

function openLargeModal(
  title: string,
  opts?: { wide?: boolean; stack?: boolean },
): { body: HTMLElement; close: () => void; footer: HTMLElement } {
  if (!opts?.stack) hideModal();
  const mask = el(
    "div",
    `nerve-dialog-mask ant-modal-mask pl-stage-modal-mask${opts?.stack ? " pl-stage-modal-stack" : ""}`,
  );
  const wrap = el("div", "nerve-dialog-wrap ant-modal-wrap");
  const modal = el(
    "div",
    `nerve-dialog ant-modal pl-stage-modal${opts?.wide ? " pl-stage-modal-wide" : ""}`,
  );
  const header = el("div", "nerve-dialog-header ant-modal-header");
  header.innerHTML = `<span>${esc(title)}</span>`;
  const closeBtn = el("button", "nerve-dialog-close ant-modal-close", "×");
  closeBtn.type = "button";
  const body = el("div", "nerve-dialog-body ant-modal-body pl-stage-modal-body");
  const footer = el("div", "nerve-dialog-footer ant-modal-footer pl-stage-modal-foot");
  const close = () => {
    if (!opts?.stack) {
      document.querySelectorAll(".pl-stage-modal-stack").forEach((n) => n.remove());
    }
    mask.remove();
  };
  closeBtn.onclick = close;
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  header.appendChild(closeBtn);
  modal.appendChild(header);
  modal.appendChild(body);
  modal.appendChild(footer);
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  document.body.appendChild(mask);
  return { body, close, footer };
}

function bindOptionGroup(root: HTMLElement, name: string): void {
  root.querySelectorAll<HTMLInputElement>(`input[name="${name}"]`).forEach((inp) => {
    inp.addEventListener("change", () => {
      root.querySelectorAll(".pl-option").forEach((lab) => {
        const r = lab.querySelector(`input[name="${name}"]`) as HTMLInputElement | null;
        lab.classList.toggle("pl-option-active", !!r?.checked);
      });
    });
  });
}

function readOptionGroup(root: HTMLElement, name: string): string {
  return (root.querySelector(`input[name="${name}"]:checked`) as HTMLInputElement | null)?.value ?? "";
}

function openExpertStageModal(
  tpl: StrategyTemplate,
  onSaved: (t: StrategyTemplate) => void,
  toast: (m: string, ok?: boolean) => void,
): void {
  const { body, close, footer } = openLargeModal(`专家防护 · ${tpl.name}`);
  body.innerHTML = `<div class="pl-modal-section"><div class="pl-modal-label">防护强度</div>${optionCards("pl-expert", EXPERT_OPTS, tpl.expert_level)}</div>`;
  footer.innerHTML = `${btn({ id: "pl-stage-expert-close", label: "取消" })}${btn({ id: "pl-stage-expert-save", label: "保存", variant: "primary" })}`;
  bindOptionGroup(body, "pl-expert");
  footer.querySelector("#pl-stage-expert-close")?.addEventListener("click", close);
  footer.querySelector("#pl-stage-expert-save")?.addEventListener("click", () => {
    void (async () => {
      const level = readOptionGroup(body, "pl-expert") || tpl.expert_level;
      try {
        const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
          `/api/protection-strategies/${encodeURIComponent(tpl.id)}`,
          { expert_level: level },
        );
        if (resp.template) onSaved(resp.template);
        toast("已保存");
        close();
      } catch (e) {
        showResult({ type: "error", title: "保存失败", message: String(e) });
      }
    })();
  });
}

function openSemanticStageModal(
  tpl: StrategyTemplate,
  onSaved: (t: StrategyTemplate) => void,
  toast: (m: string, ok?: boolean) => void,
): void {
  const { body, close, footer } = openLargeModal(`语义设置 · ${tpl.name}`);
  body.innerHTML = `<div class="pl-modal-section"><div class="pl-modal-label">语义模式</div>${optionCards("pl-semantic", SEMANTIC_OPTS, tpl.semantic_mode)}</div>`;
  footer.innerHTML = `${btn({ id: "pl-stage-sem-close", label: "取消" })}${btn({ id: "pl-stage-sem-save", label: "保存", variant: "primary" })}`;
  bindOptionGroup(body, "pl-semantic");
  footer.querySelector("#pl-stage-sem-close")?.addEventListener("click", close);
  footer.querySelector("#pl-stage-sem-save")?.addEventListener("click", () => {
    void (async () => {
      const mode = readOptionGroup(body, "pl-semantic") || tpl.semantic_mode;
      try {
        const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
          `/api/protection-strategies/${encodeURIComponent(tpl.id)}`,
          { semantic_mode: mode },
        );
        if (resp.template) onSaved(resp.template);
        toast("已保存");
        close();
      } catch (e) {
        showResult({ type: "error", title: "保存失败", message: String(e) });
      }
    })();
  });
}

function openCoreStageModal(
  tpl: StrategyTemplate,
  onSaved: (t: StrategyTemplate) => void,
  toast: (m: string, ok?: boolean) => void,
): void {
  const core = tpl.core ?? { version: 1, mode: "block", modules: {} };
  const modules = { ...(core.modules ?? {}) };

  const { body, close, footer } = openLargeModal(`核心防护 · ${tpl.name}`, { wide: true });
  const cardsHtml = orderedWebCoreModuleEntries(modules)
    .map(([key, cfg]) => {
      const meta = MODULE_META[key] ?? { title: key, desc: "" };
      return `<div class="protection-module-card app-mod-card${cfg.enabled ? "" : " mod-card-disabled"}" data-key="${esc(key)}">
      <div class="protection-module-head">
        <strong>${esc(meta.title)}</strong>
        <label class="switch"><input type="checkbox" class="mod-enabled" ${cfg.enabled ? "checked" : ""}><span></span></label>
      </div>
      <p class="muted small">${esc(meta.desc)}</p>
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
      </div>
    </div>`;
    })
    .join("");

  body.innerHTML = `
    <div class="pl-core-mode-row">
      <span class="pl-modal-label">默认动作</span>
      <select class="ant-input" id="pl-core-mode" style="width:140px">
        <option value="block" ${core.mode === "block" ? "selected" : ""}>拦截</option>
        <option value="detect" ${core.mode === "detect" ? "selected" : ""}>仅检测</option>
      </select>
    </div>
    <p class="pl-section-label">攻击类型模块</p>
    <div class="module-grid" id="pl-core-grid">${cardsHtml}</div>`;
  footer.innerHTML = `${btn({ id: "pl-core-close", label: "取消" })}${btn({ id: "pl-core-save", label: "保存", variant: "primary" })}`;

  const grid = body.querySelector("#pl-core-grid")!;
  bindModuleCardStyles(body);

  footer.querySelector("#pl-core-close")?.addEventListener("click", close);
  footer.querySelector("#pl-core-save")?.addEventListener("click", () => {
    void (async () => {
      const nextModules: typeof modules = { ...modules };
      grid.querySelectorAll<HTMLElement>(".protection-module-card").forEach((card) => {
        const key = card.dataset.key!;
        const prev = modules[key];
        nextModules[key] = {
          enabled: (card.querySelector(".mod-enabled") as HTMLInputElement).checked,
          action: (card.querySelector(".mod-action") as HTMLSelectElement).value,
          sensitivity: (card.querySelector(".mod-sensitivity") as HTMLSelectElement).value,
          disabled_rules: prev?.disabled_rules,
        };
      });
      try {
        const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
          `/api/protection-strategies/${encodeURIComponent(tpl.id)}`,
          {
            core: {
              version: core.version ?? 1,
              mode: (body.querySelector("#pl-core-mode") as HTMLSelectElement).value,
              modules: nextModules,
            },
          },
        );
        if (resp.template) onSaved(resp.template);
        toast("已保存");
        close();
      } catch (e) {
        showResult({ type: "error", title: "保存失败", message: String(e) });
      }
    })();
  });
}

function strategyCustomSetIds(tpl: StrategyTemplate): string[] {
  const raw = tpl.custom as { set_ids?: unknown } | undefined;
  if (Array.isArray(raw?.set_ids) && raw!.set_ids!.length) {
    return [...new Set(raw!.set_ids!.map((x) => String(x).trim()).filter(Boolean))];
  }
  const mods = tpl.core?.modules ?? {};
  return Object.keys(mods)
    .filter((k) => k.startsWith(CUSTOM_MODULE_KEY_PREFIX))
    .map((k) => k.slice(CUSTOM_MODULE_KEY_PREFIX.length));
}

function buildCoreWithCustomSets(
  core: StrategyTemplate["core"] | undefined,
  setIds: string[],
): NonNullable<StrategyTemplate["core"]> {
  const base = core ?? { version: 1, mode: "block", modules: {} };
  const modules: NonNullable<StrategyTemplate["core"]>["modules"] = { ...(base.modules ?? {}) };
  for (const key of Object.keys(modules)) {
    if (key.startsWith(CUSTOM_MODULE_KEY_PREFIX) || key === "custom") {
      delete modules[key];
    }
  }
  for (const id of setIds) {
    const key = customModuleKey(id);
    modules[key] = modules[key] ?? {
      enabled: true,
      action: "block",
      sensitivity: "medium",
    };
    modules[key]!.enabled = true;
  }
  return {
    version: base.version ?? 1,
    mode: base.mode ?? "block",
    modules,
  };
}

function openCustomStageModal(
  tpl: StrategyTemplate,
  onSaved: (t: StrategyTemplate) => void,
  toast: (m: string, ok?: boolean) => void,
): void {
  const { body, close, footer } = openLargeModal(`自定义检测 · ${tpl.name}`, { wide: true });
  body.innerHTML = `<div class="page-loading">加载规则集…</div>`;
  footer.innerHTML = `${btn({ id: "pl-custom-close", label: "取消" })}${btn({ id: "pl-custom-save", label: "保存", variant: "primary" })}`;

  let allSets: CustomRuleSetInfo[] = [];
  let selectedIds = strategyCustomSetIds(tpl);

  async function loadSets(): Promise<void> {
    const url = tpl.site_id
      ? `/api/custom-rules?site_id=${encodeURIComponent(tpl.site_id)}`
      : "/api/global/custom-rules";
    const res = await apiGet<{ sets: CustomRuleSetInfo[] }>(url);
    allSets = res.sets ?? [];
  }

  function openPickSetsModal(): void {
    const {
      body: pickBody,
      close: closePick,
      footer: pickFooter,
    } = openLargeModal("添加自定义规则集", { wide: true, stack: true });

    function paintPick(): void {
      const left = allSets.filter((s) => !selectedIds.includes(s.id));
      const rows = left.length
        ? left
            .map(
              (s) => `<tr data-set-id="${esc(s.id)}">
                <td><strong>${esc(s.name || s.id)}</strong><div class="muted small"><code>${esc(s.id)}</code></div></td>
                <td class="muted small">${esc(s.description || "—")}</td>
                <td>${s.enabled === false ? tag("已停用", "orange") : tag("可用", "green")}</td>
                <td class="pl-custom-ops">
                  ${btn({ label: "添加", size: "sm", variant: "primary", className: "pl-custom-pick-add", attrs: { "data-set-id": s.id } })}
                </td>
              </tr>`,
            )
            .join("")
        : `<tr><td colspan="4" class="muted">${
            allSets.length ? "可添加的规则集已全部挂到本策略。" : "暂无自定义规则集，请先到「签名规则」创建。"
          }</td></tr>`;

      pickBody.innerHTML = `
        <div class="pl-modal-section">
          <p class="muted small" style="margin:0 0 12px">来自自定义防护 / 签名规则中的规则集，选择后挂到本策略快照。</p>
          <div class="ant-table-wrapper">
            <table class="ant-table">
              <thead><tr><th>规则集</th><th>说明</th><th>状态</th><th>操作</th></tr></thead>
              <tbody>${rows}</tbody>
            </table>
          </div>
        </div>`;

      pickBody.querySelectorAll(".pl-custom-pick-add").forEach((node) => {
        node.addEventListener("click", () => {
          const id = (node as HTMLElement).dataset.setId!;
          if (!selectedIds.includes(id)) selectedIds = [...selectedIds, id];
          paint();
          paintPick();
        });
      });
    }

    pickFooter.innerHTML = `${btn({ id: "pl-custom-pick-close", label: "关闭" })}`;
    pickFooter.querySelector("#pl-custom-pick-close")?.addEventListener("click", closePick);
    paintPick();
  }

  function paint(): void {
    const selected = selectedIds
      .map((id) => allSets.find((s) => s.id === id) ?? { id, name: id, description: "", enabled: true })
      .filter(Boolean);

    const rows = selected.length
      ? selected
          .map(
            (s) => `<tr data-set-id="${esc(s.id)}">
              <td><strong>${esc(s.name || s.id)}</strong><div class="muted small"><code>${esc(s.id)}</code></div></td>
              <td class="muted small">${esc(s.description || "—")}</td>
              <td>${s.enabled === false ? tag("已停用", "orange") : tag("可用", "green")}</td>
              <td class="pl-custom-ops">
                ${btn({ label: "移除", size: "sm", className: "pl-custom-detach", attrs: { "data-set-id": s.id } })}
              </td>
            </tr>`,
          )
          .join("")
      : `<tr><td colspan="4" class="muted">尚未绑定自定义规则集，请点击「添加」从已有规则集中选择。</td></tr>`;

    body.innerHTML = `
      <div class="pl-modal-section">
        <div class="pl-modal-toolbar">
          ${btn({ id: "pl-custom-add", label: "添加", size: "sm", variant: "primary" })}
        </div>
        <div class="ant-table-wrapper">
          <table class="ant-table">
            <thead><tr><th>规则集</th><th>说明</th><th>状态</th><th>操作</th></tr></thead>
            <tbody>${rows}</tbody>
          </table>
        </div>
        <p class="muted small" style="margin-top:12px">本页仅添加/移除已有规则集到策略快照；新建与编辑请前往「签名规则」。</p>
      </div>`;

    body.querySelector("#pl-custom-add")?.addEventListener("click", () => openPickSetsModal());

    body.querySelectorAll(".pl-custom-detach").forEach((node) => {
      node.addEventListener("click", () => {
        const id = (node as HTMLElement).dataset.setId!;
        selectedIds = selectedIds.filter((x) => x !== id);
        paint();
      });
    });
  }
  footer.querySelector("#pl-custom-close")?.addEventListener("click", close);
  footer.querySelector("#pl-custom-save")?.addEventListener("click", () => {
    void (async () => {
      try {
        const core = buildCoreWithCustomSets(tpl.core, selectedIds);
        const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
          `/api/protection-strategies/${encodeURIComponent(tpl.id)}`,
          {
            custom: { set_ids: selectedIds },
            core,
          },
        );
        if (resp.template) onSaved(resp.template);
        toast("已保存");
        close();
      } catch (e) {
        await showResult({ type: "error", title: "保存失败", message: String(e) });
      }
    })();
  });

  void loadSets()
    .then(() => paint())
    .catch((e) => {
      body.innerHTML = `<div class="ant-alert ant-alert-error">加载规则集失败：${esc(String(e))}</div>`;
    });
}

export async function renderProtectionLevelsView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  root.innerHTML = `<div class="pl-page"><div class="page-header"><div><h2>防护策略</h2></div></div><div class="page-loading">加载中…</div></div>`;

  let templates: StrategyTemplate[] = [];
  try {
    const r = await apiGet<{ templates: StrategyTemplate[] }>("/api/protection-strategies");
    templates = r.templates ?? [];
  } catch (e) {
    root.innerHTML = `<div class="pl-page">
      <div class="page-header"><div><h2>防护策略</h2></div></div>
      <div class="ant-alert ant-alert-error">加载策略库失败：${esc(String(e))}</div>
    </div>`;
    return;
  }

  let editing: StrategyTemplate | null = null;

  function bindCreate(sel: string): void {
    root.querySelector(sel)?.addEventListener("click", () => {
      void (async () => {
        const values = await showPrompt({
          title: "新建策略",
          okText: "创建",
          cancelText: "取消",
          fields: [
            { id: "name", label: "名称", defaultValue: "自定义策略", wide: true },
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
          subject: `策略「${name}」`,
          confirmMessage: `创建「${name}」并将从当前全局配置拷贝模块快照。`,
          successMessage: `「${name}」已创建`,
          run: async () => {
            const resp = await apiPost<{ ok: boolean; template: StrategyTemplate }>(
              "/api/protection-strategies",
              { name, description: values.description?.trim() || "" },
            );
            if (!resp.template) throw new Error("未返回策略");
            templates = [...templates, resp.template];
            editing = resp.template;
          },
        });
        if (ok) paint();
      })();
    });
  }

  function paint(): void {
    if (editing) {
      root.innerHTML = renderStrategyModuleEditor(editing);
      root.querySelector("#pl-tpl-back")?.addEventListener("click", () => {
        editing = null;
        paint();
      });
      root.querySelectorAll<HTMLButtonElement>(".pl-stage-row").forEach((btnEl) => {
        btnEl.addEventListener("click", () => {
          const stage = btnEl.dataset.stage;
          const tpl = editing!;
          const onSaved = (t: StrategyTemplate) => {
            editing = t;
            templates = templates.map((x) => (x.id === t.id ? t : x));
            paint();
          };
          if (stage === "expert") openExpertStageModal(tpl, onSaved, toast);
          else if (stage === "semantic") openSemanticStageModal(tpl, onSaved, toast);
          else if (stage === "core") openCoreStageModal(tpl, onSaved, toast);
          else if (stage === "custom") openCustomStageModal(tpl, onSaved, toast);
        });
      });
      return;
    }

    root.innerHTML = `
      <div class="pl-page">
        <div class="page-header">
          <div><h2>防护策略</h2></div>
          <div class="page-header-actions">${btn({ id: "pl-tpl-new", label: "新建策略", variant: "primary" })}</div>
        </div>
        ${renderTemplateCards(templates, null)}
      </div>`;

    bindCreate("#pl-tpl-new");
    bindCreate("#pl-tpl-new-empty");

    root.querySelectorAll(".pl-tpl-edit").forEach((node) => {
      node.addEventListener("click", () => {
        const id = (node as HTMLElement).dataset.tplId;
        editing = templates.find((t) => t.id === id) ?? null;
        paint();
      });
    });

    root.querySelectorAll(".pl-tpl-apply").forEach((node) => {
      node.addEventListener("click", () => {
        const id = (node as HTMLElement).dataset.tplId!;
        const tpl = templates.find((t) => t.id === id);
        void (async () => {
          if (!tpl) return;
          if (!isStrategyEnabled(tpl)) {
            await showResult({ type: "error", title: "无法套用", message: "该策略已禁用" });
            return;
          }
          let sites = await loadSites();
          if (tpl.site_id) {
            sites = sites.filter((s) => s.id === tpl.site_id);
          }
          if (!sites.length) {
            await showResult({ type: "error", title: "无法套用", message: "没有可套用的应用" });
            return;
          }
          const values = await showPrompt({
            title: `套用策略「${tpl.name}」`,
            okText: "套用",
            fields: [
              {
                id: "site_id",
                label: "目标应用",
                type: "select",
                defaultValue: sites[0].id,
                options: sites.map((s) => ({
                  value: s.id,
                  label: `${s.name}（${s.id}）`,
                })),
              },
            ],
          });
          if (!values?.site_id) return;
          await runMutatingAction({
            action: "save",
            subject: `策略「${tpl.name}」→ 应用 ${values.site_id}`,
            confirmMessage: `将「${tpl.name}」的模块与语义快照写入应用「${values.site_id}」，并绑定该策略。确认继续？`,
            successMessage: `已套用到应用 ${values.site_id}`,
            run: async () => {
              await apiPost(`/api/protection-strategies/${encodeURIComponent(id)}/apply`, {
                target: "app",
                site_id: values.site_id,
              });
            },
          });
        })();
      });
    });

    root.querySelectorAll(".pl-tpl-apply-global").forEach((node) => {
      node.addEventListener("click", () => {
        const id = (node as HTMLElement).dataset.tplId!;
        const tpl = templates.find((t) => t.id === id);
        void (async () => {
          if (!tpl || tpl.site_id) return;
          await runMutatingAction({
            action: "save",
            subject: `策略「${tpl.name}」→ 全局默认`,
            confirmMessage: `将「${tpl.name}」套用为全局默认防护配置？各应用未单独绑定时将继承。`,
            successMessage: "已设为全局默认",
            run: async () => {
              await apiPost(`/api/protection-strategies/${encodeURIComponent(id)}/apply`, {
                target: "global",
              });
            },
          });
        })();
      });
    });

    root.querySelectorAll(".pl-tpl-del").forEach((node) => {
      node.addEventListener("click", () => {
        const id = (node as HTMLElement).dataset.tplId!;
        const tpl = templates.find((t) => t.id === id);
        void (async () => {
          const ok = await runMutatingAction({
            action: "delete",
            subject: `策略「${tpl?.name ?? id}」`,
            confirmMessage: `确定删除「${tpl?.name ?? id}」？`,
            successMessage: "已删除",
            run: async () => {
              await apiDelete(`/api/protection-strategies/${encodeURIComponent(id)}`);
              templates = templates.filter((t) => t.id !== id);
            },
          });
          if (ok) paint();
        })();
      });
    });
  }

  paint();
}

/** App-config drawer protection-strategy panel */
export function renderAppProtectionLevelsPanel(
  levels: ProtectionStrategy,
  templates: StrategyTemplate[] = [],
): string {
  const tplOpts = templates
    .map(
      (t) =>
        `<option value="${esc(t.id)}">${esc(t.name)}（${esc(modeLabel(t.semantic_mode))} · ${esc(levelLabel(t.expert_level))}）</option>`,
    )
    .join("");
  return `
    <div class="protection-levels-app">
      ${
        templates.length
          ? `<div class="pl-app-apply">
              <select class="ant-input" id="app-pl-tpl-sel">
                <option value="">从策略库套用到本应用…</option>
                ${tplOpts}
              </select>
              ${btn({ id: "app-pl-tpl-apply", label: "套用", size: "sm", variant: "primary" })}
            </div>`
          : ""
      }
      <p class="pl-section-label">本应用档位（独立快照）</p>
      ${renderStrategyGrid(levels, { app: true })}
    </div>`;
}

export function collectAppProtectionLevels(root: HTMLElement): {
  semantic_mode: string;
  expert_level: string;
} {
  const semantic =
    (root.querySelector("#app-pl-semantic-sel") as HTMLSelectElement | null)?.value ?? "standard";
  const expert =
    (root.querySelector("#app-pl-expert-sel") as HTMLSelectElement | null)?.value ?? "medium";
  return {
    semantic_mode: semantic,
    expert_level: expert,
  };
}

export async function fetchStrategyTemplates(siteId?: string): Promise<StrategyTemplate[]> {
  try {
    const q = siteId ? `?site_id=${encodeURIComponent(siteId)}` : "";
    const r = await apiGet<{ templates: StrategyTemplate[] }>(`/api/protection-strategies${q}`);
    return r.templates ?? [];
  } catch {
    return [];
  }
}
