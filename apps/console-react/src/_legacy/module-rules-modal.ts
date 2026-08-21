/** Legacy (pre-React) console module: Module rules modal. */
import { categoryLabel, esc, hideModal } from "./components/ui.js";
import {
  bindRiskSectionRuleToggles,
  bindRiskSectionsPagination,
  fetchRuleRows,
  normalizeCategory,
  PROTECT_LABEL,
  renderRiskSectionsHtml,
  type ProtectLevel,
} from "./crs-rules-shared.js";
import { crsRuleTogglesLimited, type CrsRuntimeStatus } from "./crs-runtime-ui.js";

function el(tag: string, cls?: string, html?: string): HTMLElement {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (html != null) n.innerHTML = html;
  return n;
}

export async function showModuleRulesModal(opts: {
  category: string;
  protect: ProtectLevel;
  moduleTitle: string;
  moduleEnabled: boolean;
  moduleKey?: string;
  siteId?: string;
  /** Editable app rule set inside the app drawer */
  editable?: boolean;
  disabledRules?: string[];
  onSave?: (disabledRules: string[]) => void;
  /** After open, scroll to and highlight a rule */
  highlightRuleId?: string;
  crsRuntime?: CrsRuntimeStatus;
}): Promise<void> {
  hideModal();
  const mask = el("div", "ant-modal-mask");
  const wrap = el("div", "ant-modal-wrap");
  const modal = el("div", "ant-modal ant-modal-xxl");

  const wasmRuleLimited = opts.crsRuntime ? crsRuleTogglesLimited(opts.crsRuntime) : false;
  const editable = opts.editable === true && !wasmRuleLimited;
  const titleSuffix = editable ? " · 本应用规则集" : " · 规则预览";
  const header = el("div", "ant-modal-header");
  header.innerHTML = `<span>${esc(opts.moduleTitle)}${titleSuffix}</span>`;
  const closeBtn = el("button", "ant-modal-close", "×");
  closeBtn.type = "button";
  const close = () => mask.remove();
  closeBtn.onclick = close;
  header.appendChild(closeBtn);

  const body = el("div", "ant-modal-body");
  body.innerHTML = `<div class="ant-empty">加载规则中…</div>`;

  const footer = el("div", "ant-modal-footer");
  if (editable) {
    footer.innerHTML = `
      <div>
        <button type="button" class="ant-btn mod-rules-cancel">取消</button>
        <button type="button" class="ant-btn ant-btn-primary mod-rules-save">确定</button>
      </div>`;
  }

  modal.append(header, body);
  if (editable) modal.appendChild(footer);
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  document.body.appendChild(mask);

  const pageState: Record<string, { page: number; pageSize: number }> = {};
  const idPrefix = "mod-rules-";
  let allRows: Awaited<ReturnType<typeof fetchRuleRows>>["allRows"] = [];
  let moduleConfigs: Awaited<ReturnType<typeof fetchRuleRows>>["moduleConfigs"] = {};
  let liEnabled = true;
  const disabledRules = new Set(opts.disabledRules ?? []);

  function renderSections(filter = ""): void {
    const wasmNote = wasmRuleLimited
      ? `<div class="ant-alert ant-alert-warning" style="margin-bottom:12px">专家防护启用时，单规则开关不作用于专家段。</div>`
      : "";
    const statusNote = !opts.moduleEnabled
      ? `<div class="ant-alert ant-alert-warning" style="margin-bottom:12px">模块已关闭</div>`
      : "";
    body.innerHTML = `
      ${wasmNote}
      ${statusNote}
      <div class="protect-level-bar" style="margin-bottom:12px">
        <span class="protect-level-label">拦截等级</span>
        <span class="protect-level-tab active">${PROTECT_LABEL[opts.protect]}</span>
      </div>
      <div class="table-toolbar" style="margin-bottom:8px">
        <input class="ant-input mod-rules-filter" placeholder="搜索 ID / 名称" style="width:220px" />
        ${editable && disabledRules.size ? `<span class="muted small">已关闭 ${disabledRules.size} 条</span>` : ""}
      </div>
      <div class="mod-rules-sections"></div>`;

    const sections = body.querySelector(".mod-rules-sections")!;
    const renderOpts = {
      category: opts.category,
      protect: opts.protect,
      allRows,
      moduleConfigs,
      filter,
      idPrefix,
      pageState,
      hideEmptyTiers: true,
      editable,
      disabledRules,
      moduleKey: opts.moduleKey,
      liEnabled,
      moduleEnabled: opts.moduleEnabled,
    };
    sections.innerHTML = renderRiskSectionsHtml(renderOpts);
    bindRiskSectionsPagination(sections, renderOpts, () => renderSections(filter));
    bindRiskSectionRuleToggles(sections, renderOpts, () => {
      renderSections(filter);
    });

    const input = body.querySelector<HTMLInputElement>(".mod-rules-filter");
    if (input) {
      input.value = filter;
      input.oninput = () => {
        for (const k of Object.keys(pageState)) pageState[k].page = 1;
        renderSections(input.value);
      };
    }

    if (opts.highlightRuleId) {
      const hid = opts.highlightRuleId.trim();
      const toggle = sections.querySelector<HTMLElement>(`[data-rule-id="${CSS.escape(hid)}"]`);
      const row = toggle?.closest("tr");
      if (row) {
        row.classList.add("rule-row-highlight");
        row.scrollIntoView({ block: "center", behavior: "smooth" });
      }
    }
  }

  if (editable) {
    footer.querySelector(".mod-rules-cancel")?.addEventListener("click", close);
    footer.querySelector(".mod-rules-save")?.addEventListener("click", () => {
      opts.onSave?.([...disabledRules].sort());
      close();
    });
  }

  try {
    const data = await fetchRuleRows(opts.siteId);
    allRows = data.allRows;
    moduleConfigs = data.moduleConfigs;
    liEnabled = data.liEnabled;
    if (!allRows.some((r) => normalizeCategory(r.category) === opts.category)) {
      body.innerHTML = `<div class="ant-empty">${categoryLabel(opts.category)} 暂无规则</div>`;
      return;
    }
    renderSections(opts.highlightRuleId?.trim() ?? "");
  } catch (e) {
    body.innerHTML = `<div class="ant-alert ant-alert-error">${esc(String(e))}</div>`;
  }
}
