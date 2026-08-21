/** Legacy (pre-React) console module: Rules custom view. */
import { apiDelete, apiGet, apiPost, apiPut } from "./api.js";
import {
  actionTag,
  bindPaginationBar,
  btn,
  defaultPaginationState,
  el,
  emptyState,
  esc,
  formField,
  formStack,
  hideModal,
  mountNerveDialog,
  nerveCheckbox,
  nerveForm,
  nerveInput,
  nerveSectionCard,
  nerveTable,
  nerveTableActions,
  NERVE_COL_ACTIONS,
  nerveTag,
  pageHeader,
  renderPaginatedSection,
  runMutatingAction,
  select,
  showPrompt,
  showResult,
  statPill,
  syncActions,
  type PaginationState,
} from "./components/ui.js";

type RuleSet = {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
};

type CustomRule = {
  id: string;
  rule_set_id: string;
  name: string;
  phase: string;
  priority: number;
  enabled: boolean;
  match_field: string;
  match_op: string;
  match_pattern: string;
  action: string;
  severity: string;
  category: string;
  rule_type?: string;
  plugin_name?: string;
};

type PluginDescriptor = {
  name: string;
  version: string;
  description: string;
  layer: string;
  type?: string;
  enabled: boolean;
};

const FIELD_OPTIONS = [
  { v: "uri", l: "URI" },
  { v: "headers", l: "请求头" },
  { v: "body", l: "请求体" },
  { v: "method", l: "Method" },
  { v: "user_agent", l: "User-Agent" },
  { v: "src_ip", l: "源 IP" },
  { v: "all", l: "全部组合" },
];

const OP_OPTIONS = [
  { v: "regex", l: "正则 (regex)" },
  { v: "contains", l: "包含 (contains)" },
  { v: "eq", l: "等于 (eq)" },
];

function ruleTypeLabel(t?: string): string {
  return t === "plugin" ? "自定义插件" : "签名规则";
}

function typeBadge(t?: string): string {
  const plugin = t === "plugin";
  return `<span class="cr-type-badge ${plugin ? "cr-type-plugin" : "cr-type-signature"}">${esc(ruleTypeLabel(t))}</span>`;
}

function statusTag(enabled: boolean): string {
  return nerveTag(enabled ? "启用" : "禁用", enabled ? "green" : "default");
}

function setBadge(name: string, unbound: boolean): string {
  if (unbound) return `<span class="cr-set-badge cr-set-unbound">未绑定</span>`;
  return `<span class="cr-set-badge">${esc(name)}</span>`;
}


function renderStats(rules: CustomRule[], sets: RuleSet[]): string {
  const unbound = rules.filter((r) => !r.rule_set_id).length;
  const plugin = rules.filter((r) => r.rule_type === "plugin").length;
  return `<div class="cr-stats-row">
    ${statPill("规则总数", rules.length, "blue")}
    ${statPill("规则集", sets.length, "blue")}
    ${statPill("未绑定", unbound, unbound ? "orange" : "green")}
    ${statPill("插件规则", plugin, "blue")}
  </div>`;
}

function renderRulesTable(rules: CustomRule[], sets: RuleSet[]): string {
  if (!rules.length) {
    return `<div class="cr-empty">${emptyState("暂无规则")}</div>`;
  }
  const body = rules
    .map((r) => {
      const detail =
        r.rule_type === "plugin"
          ? `<code class="cr-code">${esc(r.plugin_name || r.match_pattern)}</code>`
          : `<span class="cr-match mono small">${esc(r.match_field)} · ${esc(r.match_op)}</span>
             <div class="cr-pattern mono small">${esc(r.match_pattern.slice(0, 48))}${r.match_pattern.length > 48 ? "…" : ""}</div>`;
      const unbound = !r.rule_set_id;
      return `<tr>
        <td>${typeBadge(r.rule_type)}</td>
        <td><div class="cr-rule-name">${esc(r.name)}</div><code class="cr-id">${esc(r.id)}</code></td>
        <td>${detail}</td>
        <td>${actionTag(r.action)}</td>
        <td><span class="cr-priority">${r.priority}</span></td>
        <td>${setBadge(setNameById(sets, r.rule_set_id), unbound)}</td>
        <td>${statusTag(r.enabled)}</td>
        <td class="${NERVE_COL_ACTIONS}">${nerveTableActions([
          { label: "编辑", action: "edit-rule", id: r.id },
          { label: "删除", action: "del-rule", id: r.id, danger: true },
        ])}</td></tr>`;
    })
    .join("");
  return nerveTable({
    className: "cr-table",
    columns: [
      { key: "type", title: "类型" },
      { key: "name", title: "名称" },
      { key: "match", title: "匹配 / 插件" },
      { key: "action", title: "动作" },
      { key: "priority", title: "优先级" },
      { key: "set", title: "所属规则集" },
      { key: "status", title: "状态" },
      { key: "actions", title: "操作", className: NERVE_COL_ACTIONS },
    ],
    bodyHtml: body,
  });
}

function renderSetsTable(sets: RuleSet[], rules: CustomRule[]): string {
  if (!sets.length) {
    return `<div class="cr-empty">${emptyState("暂无规则集")}</div>`;
  }
  const body = sets
    .map((s) => {
      const bound = rules.filter((r) => r.rule_set_id === s.id);
      const chips =
        bound.length > 0
          ? `<div class="cr-bound-chips">${bound
              .map((r) => `<span class="cr-bound-chip">${typeBadge(r.rule_type)} ${esc(r.name)}</span>`)
              .join("")}</div>`
          : `<span class="muted small">尚未绑定规则</span>`;
      return `<tr>
        <td>
          <div class="cr-set-title">${esc(s.name)}</div>
          <code class="cr-id">${esc(s.id)}</code>
        </td>
        <td>${statusTag(s.enabled)}</td>
        <td>${chips}</td>
        <td class="muted small">${esc(s.description || "—")}</td>
        <td class="${NERVE_COL_ACTIONS}">${nerveTableActions([
          { label: "绑定规则", action: "bind-set", id: s.id, primary: true },
          { label: "编辑", action: "edit-set", id: s.id },
          { label: "删除", action: "del-set", id: s.id, danger: true },
        ])}</td></tr>`;
    })
    .join("");
  return nerveTable({
    className: "cr-table",
    columns: [
      { key: "name", title: "名称" },
      { key: "status", title: "状态" },
      { key: "bound", title: "已绑定规则" },
      { key: "desc", title: "说明" },
      { key: "actions", title: "操作", className: NERVE_COL_ACTIONS },
    ],
    bodyHtml: body,
  });
}

function setNameById(sets: RuleSet[], id: string): string {
  if (!id) return "未绑定";
  return sets.find((s) => s.id === id)?.name ?? id;
}

function ruleFormHtml(rule: Partial<CustomRule> | undefined, plugins: PluginDescriptor[]): string {
  const r = rule ?? {};
  const type = r.rule_type ?? "signature";
  const l7 = plugins.filter(
    (p) => p.layer.toLowerCase() === "l7" && String(p.type ?? "").toLowerCase() === "lua",
  );
  return nerveForm(`
    ${formField(
      "实现方式",
      select({
        id: "cr-rule-type",
        wide: true,
        options: [
          { value: "signature", label: "签名规则（正则 / 包含 / 等于）", selected: type === "signature" },
          { value: "plugin", label: "Lua 插件", selected: type === "plugin" },
        ],
      }),
    )}
    ${formField("名称", nerveInput({ id: "cr-name", value: r.name ?? "", placeholder: "规则显示名称", wide: true }))}
    <div id="cr-signature-fields" style="${type === "plugin" ? "display:none" : ""}">
      ${formField(
        "匹配字段",
        select({
          id: "cr-field",
          wide: true,
          options: FIELD_OPTIONS.map((o) => ({ value: o.v, label: o.l, selected: r.match_field === o.v })),
        }),
      )}
      ${formField(
        "匹配方式",
        select({
          id: "cr-op",
          wide: true,
          options: OP_OPTIONS.map((o) => ({ value: o.v, label: o.l, selected: r.match_op === o.v })),
        }),
      )}
      ${formField(
        "匹配模式",
        nerveInput({ id: "cr-pattern", value: r.match_pattern ?? "", placeholder: "正则或子串", wide: true }),
      )}
    </div>
    <div id="cr-plugin-fields" style="${type === "plugin" ? "" : "display:none"}">
      ${formField(
        "选择插件",
        select({
          id: "cr-plugin",
          wide: true,
          options: l7.length
            ? l7.map((p) => ({
                value: p.name,
                label: `${p.name} — ${p.description || p.version}`,
                selected: r.plugin_name === p.name,
              }))
            : [{ value: "", label: "暂无 L7 插件" }],
        }),
      )}
    </div>
    ${formField(
      "动作",
      select({
        id: "cr-action",
        wide: true,
        options: ["block", "log", "challenge"].map((a) => ({ value: a, label: a, selected: r.action === a })),
      }),
    )}
    ${formField("优先级", nerveInput({ id: "cr-priority", type: "number", value: r.priority ?? 100 }))}
    ${nerveCheckbox({ id: "cr-enabled", label: "启用", checked: r.enabled !== false })}
  `, "cr-form-inner");
}

function readRuleForm(container: HTMLElement): Record<string, unknown> {
  const type = (container.querySelector("#cr-rule-type") as HTMLSelectElement).value;
  const name = (container.querySelector("#cr-name") as HTMLInputElement).value.trim();
  if (type === "plugin") {
    const plugin = (container.querySelector("#cr-plugin") as HTMLSelectElement).value.trim();
    return {
      rule_set_id: "",
      name: name || plugin,
      rule_type: "plugin",
      plugin_name: plugin,
      match_field: "plugin",
      match_op: "plugin",
      match_pattern: plugin,
      action: (container.querySelector("#cr-action") as HTMLSelectElement).value,
      priority: Number((container.querySelector("#cr-priority") as HTMLInputElement).value) || 100,
      enabled: (container.querySelector("#cr-enabled") as HTMLInputElement).checked,
      phase: "request",
      severity: "medium",
      category: "custom",
    };
  }
  return {
    rule_set_id: "",
    name,
    rule_type: "signature",
    plugin_name: "",
    match_field: (container.querySelector("#cr-field") as HTMLSelectElement).value,
    match_op: (container.querySelector("#cr-op") as HTMLSelectElement).value,
    match_pattern: (container.querySelector("#cr-pattern") as HTMLInputElement).value.trim(),
    action: (container.querySelector("#cr-action") as HTMLSelectElement).value,
    priority: Number((container.querySelector("#cr-priority") as HTMLInputElement).value) || 100,
    enabled: (container.querySelector("#cr-enabled") as HTMLInputElement).checked,
    phase: "request",
    severity: "medium",
    category: "custom",
  };
}

function bindRuleTypeToggle(container: HTMLElement): void {
  const typeSel = container.querySelector("#cr-rule-type") as HTMLSelectElement | null;
  if (!typeSel) return;
  typeSel.onchange = () => {
    const plugin = typeSel.value === "plugin";
    const sig = container.querySelector("#cr-signature-fields") as HTMLElement;
    const plg = container.querySelector("#cr-plugin-fields") as HTMLElement;
    if (sig) sig.style.display = plugin ? "none" : "";
    if (plg) plg.style.display = plugin ? "" : "none";
  };
}

function mountModalShell(title: string, wide = false): {
  mask: HTMLElement;
  body: HTMLElement;
  footer: HTMLElement;
  close: () => void;
} {
  hideModal();
  return mountNerveDialog({ title, wide });
}

function showSimpleModal(
  title: string,
  bodyHtml: string,
  mutate: {
    action: "create" | "edit";
    subject: string;
    run: (modal: HTMLElement) => void | Promise<void>;
    validate?: (modal: HTMLElement) => string | null;
  },
): void {
  const { mask, body, footer, close } = mountModalShell(title);
  body.innerHTML = bodyHtml;
  const cancel = el("button", "nerve-btn ant-btn", "取消");
  cancel.type = "button";
  cancel.onclick = close;
  const okBtn = el("button", "nerve-btn ant-btn ant-btn-primary", "确定");
  okBtn.type = "button";
  okBtn.onclick = () => {
    void (async () => {
      const err = mutate.validate?.(mask) ?? null;
      if (err) {
        await showResult({ type: "info", title: "校验失败", message: err });
        return;
      }
      const ok = await runMutatingAction({
        action: mutate.action,
        subject: mutate.subject,
        run: () => mutate.run(mask),
      });
      if (ok) close();
    })();
  };
  footer.append(cancel, okBtn);
}

function showRuleEditorModal(
  title: string,
  rule: Partial<CustomRule> | undefined,
  plugins: PluginDescriptor[],
  handlers: {
    onSave: (modal: HTMLElement) => Promise<void | false>;
    onTest: (modal: HTMLElement) => Promise<void>;
  },
): void {
  const { mask, body, footer, close } = mountModalShell(title, true);
  body.innerHTML = `${ruleFormHtml(rule, plugins)}<pre id="cr-test-out" class="muted small" style="margin-top:8px;max-height:120px;overflow:auto;display:none"></pre>`;
  bindRuleTypeToggle(body);
  const cancel = el("button", "nerve-btn ant-btn", "取消");
  cancel.type = "button";
  cancel.onclick = close;
  const testBtn = el("button", "nerve-btn ant-btn", "测试匹配");
  testBtn.type = "button";
  testBtn.onclick = () => {
    void handlers.onTest(mask);
  };
  const saveBtn = el("button", "nerve-btn ant-btn ant-btn-primary", "保存");
  saveBtn.type = "button";
  saveBtn.onclick = () => {
    void Promise.resolve(handlers.onSave(mask)).then((stop) => {
      if (stop !== false) close();
    });
  };
  footer.append(cancel, testBtn, saveBtn);
}

export async function renderRulesCustomView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  const rulesSection = nerveSectionCard({
    title: "规则库",
    subtitle: "",
    count: "—",
    countId: "cr-rules-count",
    bodyId: "cr-rules",
  });
  const setsSection = nerveSectionCard({
    title: "规则集",
    subtitle: "",
    count: "—",
    countId: "cr-sets-count",
    bodyId: "cr-sets",
  });
  root.innerHTML = `
    <div class="cr-page">
    ${pageHeader(
      "签名规则",
      "",
      syncActions(
        btn({ id: "cr-add-rule", label: "新建规则", variant: "primary" }),
        btn({ id: "cr-add-set", label: "新建规则集" }),
        btn({ id: "cr-reload", label: "热加载" }),
        btn({ id: "cr-refresh", label: "刷新" }),
      ),
    )}
    <div id="cr-stats" class="cr-stats-placeholder muted small">加载统计…</div>
    ${rulesSection.open}加载中…${rulesSection.close}
    ${setsSection.open}加载中…${setsSection.close}
    </div>`;

  let sets: RuleSet[] = [];
  let rules: CustomRule[] = [];
  let plugins: PluginDescriptor[] = [];
  const rulesPage: PaginationState = defaultPaginationState();
  const setsPage: PaginationState = defaultPaginationState();

  function paintRulesTable(): void {
    const host = root.querySelector("#cr-rules")!;
    const section = renderPaginatedSection(
      "cr-rules",
      rules,
      rulesPage,
      (pageRules) => renderRulesTable(pageRules, sets),
      `<div class="cr-empty">${emptyState("暂无规则")}</div>`,
    );
    rulesPage.page = section.page;
    host.innerHTML = section.html;
    bindPaginationBar(host, "cr-rules", rulesPage, rules.length, (next) => {
      rulesPage.page = next.page;
      rulesPage.pageSize = next.pageSize;
      paintRulesTable();
    });
  }

  function paintSetsTable(): void {
    const host = root.querySelector("#cr-sets")!;
    const section = renderPaginatedSection(
      "cr-sets",
      sets,
      setsPage,
      (pageSets) => renderSetsTable(pageSets, rules),
      `<div class="cr-empty">${emptyState("暂无规则集")}</div>`,
    );
    setsPage.page = section.page;
    host.innerHTML = section.html;
    bindPaginationBar(host, "cr-sets", setsPage, sets.length, (next) => {
      setsPage.page = next.page;
      setsPage.pageSize = next.pageSize;
      paintSetsTable();
    });
  }

  async function loadPlugins(): Promise<void> {
    try {
      const res = await apiGet<{ plugins: PluginDescriptor[] }>("/api/plugins");
      plugins = res.plugins ?? [];
    } catch {
      plugins = [];
    }
  }

  function rulesInSet(setId: string): CustomRule[] {
    return rules.filter((r) => r.rule_set_id === setId);
  }

  async function refresh(): Promise<void> {
    const data = await apiGet<{ sets: RuleSet[]; rules: CustomRule[] }>("/api/global/custom-rules");
    sets = data.sets ?? [];
    rules = (data.rules ?? []).map((r) => ({
      ...r,
      rule_type: r.rule_type ?? "signature",
      rule_set_id: r.rule_set_id ?? "",
    }));

    const statsEl = root.querySelector("#cr-stats");
    if (statsEl) {
      statsEl.className = "cr-stats-row-wrap";
      statsEl.innerHTML = renderStats(rules, sets);
    }
    const rulesCount = root.querySelector("#cr-rules-count");
    if (rulesCount) rulesCount.textContent = String(rules.length);
    const setsCount = root.querySelector("#cr-sets-count");
    if (setsCount) setsCount.textContent = String(sets.length);

    paintRulesTable();
    paintSetsTable();
  }

  function openRuleEditor(rule?: CustomRule): void {
    const editingId = rule?.id ?? null;
    showRuleEditorModal(
      rule ? `编辑规则 ${rule.id}` : "新建规则",
      rule,
      plugins,
      {
        onSave: async (modal) => {
          const body = readRuleForm(modal);
          if (!body.name) {
            await showResult({ type: "info", title: "校验失败", message: "请填写名称" });
            return false;
          }
          if (body.rule_type === "plugin" && !body.plugin_name) {
            await showResult({ type: "info", title: "校验失败", message: "请选择自定义插件" });
            return false;
          }
          if (body.rule_type === "signature" && !body.match_pattern) {
            await showResult({ type: "info", title: "校验失败", message: "请填写匹配模式" });
            return false;
          }
          const ok = await runMutatingAction({
            action: editingId ? "edit" : "create",
            subject: String(body.name),
            run: async () => {
              if (editingId) {
                const prev = rules.find((r) => r.id === editingId);
                await apiPut(`/api/global/custom-rules/rules/${encodeURIComponent(editingId)}`, {
                  ...body,
                  rule_set_id: prev?.rule_set_id ?? "",
                });
              } else {
                await apiPost("/api/global/custom-rules/rules", body);
              }
              await refresh();
            },
          });
          return ok ? undefined : false;
        },
        onTest: async (modal) => {
          const body = readRuleForm(modal);
          if (body.rule_type === "plugin") {
            toast("插件型规则请部署后在 Gateway 环境测试", false);
            return;
          }
          const values = await showPrompt({
            title: "测试匹配",
            fields: [
              { id: "uri", label: "测试 URI", defaultValue: "/scan?tool=sqlmap", wide: true },
              {
                id: "headers",
                label: "测试 Headers",
                type: "textarea",
                placeholder: "可选，如 X-Forwarded-For: 1.2.3.4",
                rows: 2,
              },
            ],
          });
          if (!values) return;
          const uri = values.uri;
          const headers = values.headers;
          try {
            const res = await apiPost<{ hit: unknown }>("/api/global/custom-rules/test", { ...body, uri, headers, body: "" });
            const out = modal.querySelector("#cr-test-out") as HTMLElement;
            out.style.display = "";
            out.textContent = JSON.stringify(res.hit, null, 2);
          } catch (err) {
            toast(String(err), false);
          }
        },
      },
    );
  }

  function openBindModal(setId: string): void {
    const set = sets.find((s) => s.id === setId);
    if (!set) return;
    const bound = new Set(rulesInSet(setId).map((r) => r.id));
    const listHtml = rules.length
      ? `<div class="nerve-bind-list cr-bind-list">${rules
           .map((r) => {
             const checked = bound.has(r.id);
             return `<label class="nerve-bind-item cr-bind-item${checked ? " is-checked" : ""}">
               ${nerveCheckbox({ className: "cr-bind-check", value: r.id, checked, attrs: { "data-bind-id": r.id } })}
               <div class="cr-bind-item-body">
                 <div class="cr-bind-item-head">
                   ${typeBadge(r.rule_type)}
                   <strong>${esc(r.name)}</strong>
                 </div>
                 <code class="cr-id">${esc(r.id)}</code>
               </div>
             </label>`;
           })
           .join("")}</div>`
      : `<div class="cr-empty">${emptyState("暂无规则")}</div>`;
    const { mask, body, footer, close } = mountModalShell(`绑定规则 — ${set.name}`, true);
    body.innerHTML = listHtml;
    body.querySelectorAll(".nerve-bind-item").forEach((item) => {
      const inp = item.querySelector(".cr-bind-check") as HTMLInputElement;
      inp?.addEventListener("change", () => {
        item.classList.toggle("is-checked", inp.checked);
      });
    });
    const cancel = el("button", "nerve-btn ant-btn", "取消");
    cancel.type = "button";
    cancel.onclick = close;
    const okBtn = el("button", "nerve-btn ant-btn ant-btn-primary", "确定");
    okBtn.type = "button";
    okBtn.onclick = () => {
      void (async () => {
        if (!rules.length) return;
        const ids = [...mask.querySelectorAll<HTMLInputElement>(".cr-bind-check:checked")].map((c) => c.value);
        const ok = await runMutatingAction({
          action: "save",
          subject: `规则集绑定（${ids.length} 条）`,
          run: async () => {
            await apiPut(`/api/global/custom-rules/sets/${encodeURIComponent(setId)}/bind`, { rule_ids: ids });
            await refresh();
          },
        });
        if (ok) close();
      })();
    };
    footer.append(cancel, okBtn);
    void mask;
  }

  function openSetEditor(set?: RuleSet): void {
    const s = set ?? { id: "", name: "", description: "", enabled: true };
    showSimpleModal(
      set ? "编辑规则集" : "新建规则集",
      `<div class="nerve-form form-stack">
        ${formField("名称", nerveInput({ id: "cs-name", value: s.name, wide: true }))}
        ${formField("说明", nerveInput({ id: "cs-desc", value: s.description, wide: true }))}
        ${nerveCheckbox({ id: "cs-enabled", label: "启用", checked: s.enabled })}
      </div>`,
      {
        action: set ? "edit" : "create",
        subject: set?.name || "规则集",
        validate: (modal) => {
          const name = (modal.querySelector("#cs-name") as HTMLInputElement).value.trim();
          return name ? null : "请填写名称";
        },
        run: async (modal) => {
          const body = {
            name: (modal.querySelector("#cs-name") as HTMLInputElement).value.trim(),
            description: (modal.querySelector("#cs-desc") as HTMLInputElement).value.trim(),
            enabled: (modal.querySelector("#cs-enabled") as HTMLInputElement).checked,
          };
          if (set) {
            await apiPut(`/api/global/custom-rules/sets/${encodeURIComponent(set.id)}`, body);
          } else {
            await apiPost("/api/global/custom-rules/sets", body);
          }
          await refresh();
        },
      },
    );
  }

  root.addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>("[data-nerve-action]");
    if (!btn) return;
    const action = btn.getAttribute("data-nerve-action");
    const id = btn.dataset.id ?? "";
    if (action === "edit-rule") {
      const rule = rules.find((x) => x.id === id);
      if (rule) openRuleEditor(rule);
      return;
    }
    if (action === "del-rule") {
      const ok = await runMutatingAction({
        action: "delete",
        subject: id,
        confirmMessage: `确定删除规则 ${id} 吗？`,
        successMessage: `规则 ${id} 已删除`,
        run: async () => {
          await apiDelete(`/api/global/custom-rules/rules/${encodeURIComponent(id)}`);
          await refresh();
        },
      });
      void ok;
      return;
    }
    if (action === "bind-set") {
      openBindModal(id);
      return;
    }
    if (action === "edit-set") {
      const set = sets.find((x) => x.id === id);
      if (set) openSetEditor(set);
      return;
    }
    if (action === "del-set") {
      const set = sets.find((x) => x.id === id);
      const ok = await runMutatingAction({
        action: "delete",
        subject: set?.name ?? id,
        confirmMessage: `确定删除规则集「${set?.name ?? id}」吗？规则将回到未绑定状态。`,
        successMessage: `规则集「${set?.name ?? id}」已删除`,
        run: async () => {
          await apiDelete(`/api/global/custom-rules/sets/${encodeURIComponent(id)}`);
          await refresh();
        },
      });
      void ok;
    }
  });

  root.querySelector("#cr-add-rule")?.addEventListener("click", () => openRuleEditor());
  root.querySelector("#cr-add-set")?.addEventListener("click", () => openSetEditor());
  root.querySelector("#cr-reload")?.addEventListener("click", () => {
    void runMutatingAction({
      action: "save",
      subject: "自定义规则热加载",
      confirmMessage: "确定热加载自定义规则吗？",
      successMessage: "自定义规则已热加载",
      run: async () => {
        await apiPost("/api/global/custom-rules/reload", {});
        await loadPlugins();
      },
    });
  });
  root.querySelector("#cr-refresh")?.addEventListener("click", () => {
    void refresh().catch((err) => toast(String(err), false));
  });

  await loadPlugins();
  await refresh();
}
