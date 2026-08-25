/** Legacy console module: Custom set rules modal. */
import { apiGet, apiPut } from "./api.js";
import {
  actionTag,
  bindPaginationBar,
  btn,
  defaultPaginationState,
  el,
  esc,
  hideModal,
  renderPaginatedSection,
  runMutatingAction,
  searchInput,
  type PaginationState,
} from "./components/ui.js";
import { siteQuery } from "./sites.js";

type CustomRuleRow = {
  id: string;
  rule_set_id: string;
  name: string;
  rule_type?: string;
  plugin_name?: string;
  match_field: string;
  match_op: string;
  match_pattern: string;
  action: string;
  priority: number;
  enabled: boolean;
};

type CustomRulesPayload = {
  sets: { id: string; name: string; description?: string; enabled: boolean }[];
  rules: CustomRuleRow[];
};

function ruleTypeLabel(t?: string): string {
  return t === "plugin" ? "自定义插件" : "签名规则";
}

/** View / edit rules bound in a rule set (app-level enable/disable) */
export async function showCustomSetRulesModal(opts: {
  setId: string;
  setName: string;
  siteId?: string;
  moduleEnabled?: boolean;
  /** Explicit flag; editable by default when siteId is present */
  editable?: boolean;
  onSaved?: () => void;
}): Promise<void> {
  hideModal();
  const editable = opts.editable ?? Boolean(opts.siteId);
  const mask = el("div", "ant-modal-mask");
  const wrap = el("div", "ant-modal-wrap");
  const modal = el("div", "ant-modal ant-modal-wide");

  const titleSuffix = editable ? " · 本应用规则集" : " · 规则预览";
  const header = el("div", "ant-modal-header");
  header.innerHTML = `<span>${esc(opts.setName)}${titleSuffix}</span>`;
  const closeBtn = el("button", "ant-modal-close", "×");
  closeBtn.type = "button";
  const close = () => mask.remove();
  closeBtn.onclick = close;
  header.appendChild(closeBtn);

  const body = el("div", "ant-modal-body");
  body.innerHTML = `<div class="ant-empty">加载规则…</div>`;

  const footer = el("div", "ant-modal-footer");
  if (editable) {
    footer.innerHTML = `
      <div class="modal-footer-actions">
        <button type="button" class="ant-btn mod-rules-cancel">取消</button>
        <button type="button" class="ant-btn ant-btn-primary mod-rules-save">确定</button>
      </div>`;
  } else {
    footer.innerHTML = `
      <span></span>
      <div class="modal-footer-actions">
        <button type="button" class="ant-btn ant-btn-primary mod-rules-close">关闭</button>
      </div>`;
  }

  modal.append(header, body, footer);
  modal.addEventListener("click", (e) => e.stopPropagation());
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  document.body.appendChild(mask);

  footer.querySelector(".mod-rules-close")?.addEventListener("click", close);
  footer.querySelector(".mod-rules-cancel")?.addEventListener("click", close);

  let allData: CustomRulesPayload | null = null;
  const enabledState = new Map<string, boolean>();
  const rulesPage: PaginationState = defaultPaginationState();
  let filterText = "";

  function renderTable(filter = filterText): void {
    filterText = filter;
    if (!allData) return;
    const q = filter.trim().toLowerCase();
    const rules = allData.rules
      .filter((r) => r.rule_set_id === opts.setId)
      .filter((r) => {
        if (!q) return true;
        return (
          r.id.toLowerCase().includes(q) ||
          r.name.toLowerCase().includes(q) ||
          (r.plugin_name ?? "").toLowerCase().includes(q) ||
          r.match_pattern.toLowerCase().includes(q)
        );
      });

    const moduleNote =
      editable && opts.moduleEnabled === false
        ? `<div class="ant-alert ant-alert-warning" style="margin-bottom:12px">规则集已关闭</div>`
        : "";

    if (!rules.length) {
      body.innerHTML = `
        ${moduleNote}
        <p class="muted">此规则集尚未绑定规则。</p>`;
      return;
    }

    const offCount = rules.filter((r) => !(enabledState.get(r.id) ?? r.enabled)).length;
    const section = renderPaginatedSection("mod-rules", rules, rulesPage, (pageRules) => {
      const rows = pageRules
        .map((r) => {
          const on = enabledState.get(r.id) ?? r.enabled;
          const detail =
            r.rule_type === "plugin"
              ? esc(r.plugin_name || r.match_pattern)
              : `${esc(r.match_field)} · ${esc(r.match_op)} · ${esc(r.match_pattern.slice(0, 48))}${r.match_pattern.length > 48 ? "…" : ""}`;
          const statusCell = editable
            ? `<td class="col-toggle"><label class="switch switch-sm" title="本应用启用/关闭此规则">
                <input type="checkbox" class="custom-rule-toggle" data-rule-id="${esc(r.id)}" ${on ? "checked" : ""} /><span></span>
              </label></td>`
            : `<td>${on ? "启用" : "禁用"}</td>`;
          return `<tr class="${on ? "" : "rule-row-off"}">
            <td>${esc(ruleTypeLabel(r.rule_type))}</td>
            <td><code class="small">${esc(r.id)}</code><br>${esc(r.name)}</td>
            <td class="small mono">${detail}</td>
            <td>${actionTag(r.action)}</td>
            <td>${r.priority}</td>
            ${statusCell}
          </tr>`;
        })
        .join("");
      return `<div class="rule-table-wrap">
        <table class="data-table mod-rules-table">
          <thead><tr>
            <th>类型</th><th>名称</th><th>匹配 / 插件</th><th>动作</th><th>优先级</th>${editable ? `<th class="col-toggle">启用</th>` : `<th>状态</th>`}
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>`;
    });
    rulesPage.page = section.page;

    body.innerHTML = `
      ${moduleNote}
      <div class="table-toolbar" style="margin-bottom:8px">
        ${searchInput({ className: "custom-rules-filter", placeholder: "搜索 ID / 名称", value: filter })}
        ${editable && offCount ? `<span class="muted small custom-rules-off-count">已关闭 ${offCount} 条</span>` : ""}
      </div>
      <div id="mod-rules-paged">${section.html}</div>`;

    const paged = body.querySelector("#mod-rules-paged")!;
    bindPaginationBar(paged, "mod-rules", rulesPage, rules.length, (next) => {
      rulesPage.page = next.page;
      rulesPage.pageSize = next.pageSize;
      renderTable(filter);
    });

    body.querySelectorAll<HTMLInputElement>(".custom-rule-toggle").forEach((input) => {
      input.onchange = () => {
        const id = input.dataset.ruleId!;
        enabledState.set(id, input.checked);
        input.closest("tr")?.classList.toggle("rule-row-off", !input.checked);
        const rules = (allData?.rules ?? []).filter((r) => r.rule_set_id === opts.setId);
        const offCount = rules.filter((r) => !(enabledState.get(r.id) ?? r.enabled)).length;
        const hint = body.querySelector(".custom-rules-off-count");
        if (hint) {
          hint.textContent = offCount ? `已关闭 ${offCount} 条` : "";
        } else if (offCount) {
          body.querySelector(".table-toolbar")?.insertAdjacentHTML(
            "beforeend",
            `<span class="muted small custom-rules-off-count">已关闭 ${offCount} 条</span>`,
          );
        }
      };
    });

    const filterInput = body.querySelector<HTMLInputElement>(".custom-rules-filter");
    if (filterInput) {
      filterInput.oninput = () => {
        rulesPage.page = 1;
        renderTable(filterInput.value);
      };
    }
  }

  footer.querySelector(".mod-rules-save")?.addEventListener("click", () => {
    if (!editable || !allData || !opts.siteId) return;
    void (async () => {
      const next: CustomRulesPayload = {
        sets: allData!.sets,
        rules: allData!.rules.map((r) => {
          if (r.rule_set_id !== opts.setId && !enabledState.has(r.id)) return r;
          const enabled = enabledState.has(r.id) ? enabledState.get(r.id)! : r.enabled;
          return { ...r, enabled };
        }),
      };
      const ok = await runMutatingAction({
        action: "save",
        subject: opts.setName,
        confirmMessage: `确定保存规则集「${opts.setName}」的启用状态吗？`,
        successMessage: `规则集「${opts.setName}」已保存`,
        run: async () => {
          await apiPut(`/api/custom-rules${siteQuery(opts.siteId!)}`, next);
          opts.onSaved?.();
        },
      });
      if (ok) close();
    })();
  });

  try {
    const qs = opts.siteId ? siteQuery(opts.siteId) : "";
    allData = await apiGet<CustomRulesPayload>(`/api/custom-rules${qs}`);
    for (const r of allData.rules ?? []) {
      if (r.rule_set_id === opts.setId) enabledState.set(r.id, r.enabled);
    }
    renderTable();
  } catch (e) {
    body.innerHTML = `<div class="ant-alert ant-alert-error">${esc(e instanceof Error ? e.message : String(e))}</div>`;
  }
}
