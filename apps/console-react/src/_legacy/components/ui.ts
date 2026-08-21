/** Legacy console shared UI helpers. */
export { esc, el, htmlAttrs } from "./esc.js";
export {
  alertBox,
  btn,
  card,
  filterSelect,
  formField,
  formLabel,
  formStack,
  inlineField,
  input,
  panelCard,
  searchInput,
  select,
  syncActions,
  tabBar,
  tableToolbar,
  tag,
  textarea,
  type AlertTone,
  type BtnOptions,
  type BtnSize,
  type BtnVariant,
  type CardOptions,
  type InputOptions,
  type PanelCardOptions,
  type SearchInputOptions,
  type SelectOption,
  type SelectOptions,
  type TabItem,
  type TagTone,
  type TextareaOptions,
} from "./controls.js";

export * from "./nerve.js";

import { formField, formStack, input, select, textarea, type SelectOption } from "./controls.js";
import { esc, el } from "./esc.js";

export function pageHeader(title: string, subtitle: string, actionsHtml = ""): string {
  return `
    <div class="page-header">
      <div><h2>${title}</h2>${subtitle ? `<p>${subtitle}</p>` : ""}</div>
      ${actionsHtml ? `<div class="page-header-actions">${actionsHtml}</div>` : ""}
    </div>`;
}

export function emptyState(
  msg: string,
  opts?: { description?: string; className?: string },
): string {
  const cls = ["ant-empty", opts?.className?.trim()].filter(Boolean).join(" ");
  const icon = `<svg class="ant-empty-img" viewBox="0 0 64 41" width="64" height="41" aria-hidden="true" focusable="false">
    <g fill="none" fill-rule="evenodd">
      <g transform="translate(0 1)">
        <ellipse fill="#f5f5f5" cx="32" cy="33" rx="32" ry="7"/>
        <g fill-rule="nonzero" stroke="#d9d9d9">
          <path d="M55 12.76L44.854 1.258C44.367.474 43.656 0 42.907 0H21.093c-.749 0-1.46.474-1.947 1.257L9 12.761V22h46v-9.24z"/>
          <path fill="#fafafa" d="M41.613 15.931c0-1.605.994-2.93 2.227-2.931H55v18.137C55 33.26 53.68 35 52.05 35h-40.1C10.32 35 9 33.259 9 31.137V13h11.16c1.233 0 2.227 1.323 2.227 2.928v.022c0 1.605 1.005 2.901 2.237 2.901h14.752c1.232 0 2.237-1.308 2.237-2.913v-.007z" stroke="none"/>
        </g>
      </g>
    </g>
  </svg>`;
  const desc = opts?.description?.trim()
    ? `<p class="ant-empty-hint">${esc(opts.description.trim())}</p>`
    : "";
  return `<div class="${cls}">
    <div class="ant-empty-image">${icon}</div>
    <div class="ant-empty-description">${esc(msg)}</div>
    ${desc}
  </div>`;
}

export function switchHtml(id: string, checked: boolean, label = ""): string {
  return `<label class="switch-wrap">${label ? `<span>${esc(label)}</span>` : ""}
    <span class="switch"><input type="checkbox" id="${id}" ${checked ? "checked" : ""} /><span></span></span></label>`;
}

export function statPill(label: string, value: string | number, tone: "blue" | "red" | "green" | "orange" = "blue"): string {
  return `<div class="stat-pill stat-${tone}"><div class="stat-pill-val">${value}</div><div class="stat-pill-lbl">${esc(label)}</div></div>`;
}

export function actionTag(action: string): string {
  const cls = action === "block" ? "ant-tag-red" : action === "log" ? "ant-tag-orange" : "ant-tag-green";
  const label = action === "block" ? "拦截" : action === "log" ? "记录" : action;
  return `<span class="ant-tag ${cls}">${esc(label)}</span>`;
}

export const CATEGORY_LABEL: Record<string, string> = {
  sqli: "SQL 注入",
  xss: "XSS",
  rce: "命令注入",
  path_traversal: "路径穿越",
  ssrf: "SSRF",
  webshell: "Webshell",
  behavior: "行为异常",
  bot: "Bot",
  geo: "地域",
  intel: "威胁情报",
  misc: "其他",
  protocol: "协议",
  l3_policy: "L3 策略",
  anomaly: "异常行为",
  orphan_connect: "孤儿连接",
  port_scan: "端口扫描",
  baseline_violation: "基线违规",
  escalation: "升级封禁",
  custom: "自定义规则",
  expert: "专家防护",
  crs: "专家防护",
  prompt_injection: "Prompt 注入",
  ai_artifact: "AI 配置泄露",
};

export function categoryLabel(c: string): string {
  return CATEGORY_LABEL[c] ?? c;
}

export type ConfirmOptions = {
  title: string;
  message: string;
  okText?: string;
  cancelText?: string;
  danger?: boolean;
};

let modalRoot: HTMLElement | null = null;

export function hideModal(): void {
  modalRoot?.remove();
  modalRoot = null;
  document.querySelectorAll(".ant-modal-mask").forEach((el) => el.remove());
}

export type PromptField = {
  id: string;
  label: string;
  type?: "text" | "textarea" | "select";
  defaultValue?: string;
  placeholder?: string;
  wide?: boolean;
  rows?: number;
  options?: SelectOption[];
};

export type PromptOptions = {
  title: string;
  fields: PromptField[];
  okText?: string;
  cancelText?: string;
};

/** Form input dialog (can stack above other modals without closing the underlying one) */
export function showPrompt(opts: PromptOptions): Promise<Record<string, string> | null> {
  return new Promise((resolve) => {
    const mask = el("div", "nerve-dialog-mask ant-modal-mask");
    const wrap = el("div", "nerve-dialog-wrap ant-modal-wrap");
    const modal = el("div", "nerve-dialog ant-modal");
    const header = el("div", "ant-modal-header");
    header.innerHTML = `<span>${esc(opts.title)}</span>`;
    const closeBtn = el("button", "nerve-dialog-close ant-modal-close", "×");
    closeBtn.type = "button";
    const finish = (values: Record<string, string> | null) => {
      mask.remove();
      resolve(values);
    };
    closeBtn.onclick = () => finish(null);
    header.appendChild(closeBtn);

    const body = el("div", "nerve-dialog-body ant-modal-body");
    const fieldsHtml = opts.fields
      .map((f) => {
        let control: string;
        if (f.type === "textarea") {
          control = textarea({
            id: f.id,
            value: f.defaultValue ?? "",
            placeholder: f.placeholder,
            wide: f.wide !== false,
            rows: f.rows ?? 3,
          });
        } else if (f.type === "select") {
          const options = (f.options ?? []).map((o) => ({
            ...o,
            selected: o.selected ?? o.value === (f.defaultValue ?? ""),
          }));
          control = select({ id: f.id, wide: f.wide !== false, options });
        } else {
          control = input({
            id: f.id,
            value: f.defaultValue ?? "",
            placeholder: f.placeholder,
            wide: f.wide,
          });
        }
        return formField(f.label, control);
      })
      .join("");
    body.innerHTML = formStack(fieldsHtml);

    const footer = el("div", "nerve-dialog-footer ant-modal-footer");
    const cancel = el("button", "nerve-btn ant-btn", opts.cancelText ?? "取消");
    cancel.type = "button";
    cancel.onclick = () => finish(null);
    const okBtn = el("button", "nerve-btn ant-btn ant-btn-primary", opts.okText ?? "确定");
    okBtn.type = "button";
    okBtn.onclick = () => {
      const values: Record<string, string> = {};
      for (const f of opts.fields) {
        const fieldEl = body.querySelector(`#${CSS.escape(f.id)}`) as
          | HTMLInputElement
          | HTMLTextAreaElement
          | HTMLSelectElement
          | null;
        values[f.id] = fieldEl?.value ?? "";
      }
      finish(values);
    };
    footer.append(cancel, okBtn);

    modal.append(header, body, footer);
    wrap.appendChild(modal);
    mask.appendChild(wrap);
    modal.addEventListener("click", (e) => e.stopPropagation());
    mask.addEventListener("click", (e) => {
      if (e.target === mask) finish(null);
    });
    document.body.appendChild(mask);
    (body.querySelector("input, textarea") as HTMLElement | null)?.focus();
  });
}

/** Confirm dialog matching Console Ant Design styling */
export function showConfirm(opts: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    hideModal();
    modalRoot = el("div", "nerve-dialog-mask ant-modal-mask");
    const wrap = el("div", "nerve-dialog-wrap ant-modal-wrap");
    const modal = el("div", "nerve-dialog ant-modal");
    const header = el("div", "nerve-dialog-header ant-modal-header");
    header.innerHTML = `<span>${esc(opts.title)}</span>`;
    const closeBtn = el("button", "nerve-dialog-close ant-modal-close", "×");
    closeBtn.type = "button";
    const finish = (ok: boolean) => {
      hideModal();
      resolve(ok);
    };
    closeBtn.onclick = () => finish(false);
    header.appendChild(closeBtn);

    const body = el("div", "nerve-dialog-body ant-modal-body");
    body.innerHTML = `<p class="confirm-message">${esc(opts.message).replace(/\n/g, "<br>")}</p>`;

    const footer = el("div", "nerve-dialog-footer ant-modal-footer");
    const cancel = el("button", "nerve-btn ant-btn", opts.cancelText ?? "取消");
    cancel.type = "button";
    cancel.onclick = () => finish(false);
    const okBtn = el(
      "button",
      opts.danger ? "nerve-btn ant-btn ant-btn-danger" : "nerve-btn ant-btn ant-btn-primary",
      opts.okText ?? "确定",
    );
    okBtn.type = "button";
    okBtn.onclick = () => finish(true);
    footer.append(cancel, okBtn);

    modal.append(header, body, footer);
    wrap.appendChild(modal);
    modalRoot.appendChild(wrap);
    modalRoot.addEventListener("click", (e) => {
      if (e.target === modalRoot) finish(false);
    });
    document.body.appendChild(modalRoot);
    okBtn.focus();
  });
}

/** Table pagination: 5 / 10 / 20 / 50 / 100 options, default 10 */
export const PAGE_SIZE_OPTIONS = [5, 10, 20, 50, 100] as const;
export const DEFAULT_PAGE_SIZE = 10;

export type PaginationState = { page: number; pageSize: number };

export function paginate<T>(items: T[], page: number, pageSize: number): T[] {
  const start = (page - 1) * pageSize;
  return items.slice(start, start + pageSize);
}

export function paginationTotalPages(total: number, pageSize: number): number {
  return Math.max(1, Math.ceil(Math.max(total, 1) / pageSize));
}

export function clampPage(page: number, total: number, pageSize: number): number {
  const pages = paginationTotalPages(total, pageSize);
  return Math.min(Math.max(1, page), pages);
}

export function renderPaginationBar(
  prefix: string,
  total: number,
  page: number,
  pageSize: number,
): string {
  const pages = paginationTotalPages(total, pageSize);
  const from = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const to = Math.min(page * pageSize, total);
  const sizeOpts = PAGE_SIZE_OPTIONS.map(
    (n) => `<option value="${n}" ${n === pageSize ? "selected" : ""}>${n}</option>`,
  ).join("");
  return `
    <div class="nerve-pagination table-pagination">
      <span class="muted">显示 ${from}–${to}，共 ${total} 条</span>
      <div class="table-pagination-actions">
        <label class="muted">每页
          <select class="nerve-select ant-input page-size-select" id="${prefix}-page-size" style="width:72px;margin-left:4px">${sizeOpts}</select>
        </label>
        <button type="button" class="nerve-btn ant-btn ant-btn-sm" id="${prefix}-page-prev" ${page <= 1 ? "disabled" : ""}>上一页</button>
        <span class="muted page-indicator">${page} / ${pages}</span>
        <button type="button" class="nerve-btn ant-btn ant-btn-sm" id="${prefix}-page-next" ${page >= pages ? "disabled" : ""}>下一页</button>
      </div>
    </div>`;
}

export function bindPaginationBar(
  root: HTMLElement,
  prefix: string,
  state: PaginationState,
  total: number,
  onChange: (next: PaginationState) => void,
): void {
  root.querySelector(`#${prefix}-page-size`)?.addEventListener("change", (e) => {
    const pageSize = Number((e.target as HTMLSelectElement).value);
    onChange({ page: 1, pageSize });
  });
  root.querySelector(`#${prefix}-page-prev`)?.addEventListener("click", () => {
    if (state.page > 1) onChange({ ...state, page: state.page - 1 });
  });
  root.querySelector(`#${prefix}-page-next`)?.addEventListener("click", () => {
    const pages = paginationTotalPages(total, state.pageSize);
    if (state.page < pages) onChange({ ...state, page: state.page + 1 });
  });
}

/** Standard data-table HTML (works with `.data-table` / `.ant-table-wrapper` styles) */
export type DataTableColumn<T> = {
  key: string;
  title: string;
  className?: string;
  render: (row: T) => string;
};

export function renderDataTable<T>(opts: {
  columns: DataTableColumn<T>[];
  rows: T[];
  tableClass?: string;
  wrapClass?: string;
  emptyText?: string;
  rowClass?: (row: T) => string;
}): string {
  const tableClass = opts.tableClass ?? "data-table";
  const wrapClass = opts.wrapClass ?? "ant-table-wrapper";
  if (!opts.rows.length) {
    return `<div class="${wrapClass}">${emptyState(opts.emptyText ?? "暂无数据")}</div>`;
  }
  const head = opts.columns
    .map((c) => `<th${c.className ? ` class="${c.className}"` : ""}>${esc(c.title)}</th>`)
    .join("");
  const body = opts.rows
    .map((row) => {
      const cls = opts.rowClass?.(row) ?? "";
      const cells = opts.columns.map((c) => `<td>${c.render(row)}</td>`).join("");
      return `<tr${cls ? ` class="${cls}"` : ""}>${cells}</tr>`;
    })
    .join("");
  return `<div class="${wrapClass}"><table class="${tableClass}"><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table></div>`;
}

export type ResultOptions = {
  type: "success" | "error" | "info";
  title: string;
  message: string;
  okText?: string;
};

const MUTATE_VERB: Record<"create" | "edit" | "save" | "delete", string> = {
  create: "新建",
  edit: "编辑",
  save: "保存",
  delete: "删除",
};

/** Result dialog (success / failure / info) */
export function showResult(opts: ResultOptions): Promise<void> {
  return new Promise((resolve) => {
    hideModal();
    modalRoot = el("div", "ant-modal-mask");
    const wrap = el("div", "ant-modal-wrap");
    const modal = el("div", "ant-modal ant-modal-sm nerve-result-modal");
    const header = el("div", "ant-modal-header");
    header.innerHTML = `<span>${esc(opts.title)}</span>`;
    const closeBtn = el("button", "nerve-dialog-close ant-modal-close", "×");
    closeBtn.type = "button";
    const finish = () => {
      hideModal();
      resolve();
    };
    closeBtn.onclick = finish;
    header.appendChild(closeBtn);

    const icon =
      opts.type === "success" ? "✓" : opts.type === "error" ? "✕" : "ℹ";
    const body = el("div", "nerve-dialog-body ant-modal-body");
    body.innerHTML = `
      <div class="nerve-result-body nerve-result-${opts.type}">
        <div class="nerve-result-icon" aria-hidden="true">${icon}</div>
        <p class="nerve-result-message">${esc(opts.message).replace(/\n/g, "<br>")}</p>
      </div>`;

    const footer = el("div", "ant-modal-footer nerve-result-footer");
    const okBtn = el("button", "nerve-btn ant-btn ant-btn-primary", opts.okText ?? "知道了");
    okBtn.type = "button";
    okBtn.onclick = finish;
    const actions = el("div", "modal-footer-actions");
    actions.appendChild(okBtn);
    footer.appendChild(actions);

    modal.append(header, body, footer);
    wrap.appendChild(modal);
    modalRoot.appendChild(wrap);
    modalRoot.addEventListener("click", (e) => {
      if (e.target === modalRoot) finish();
    });
    document.body.appendChild(modalRoot);
    okBtn.focus();
  });
}

export type MutateActionOptions = {
  action: "create" | "edit" | "save" | "delete";
  subject?: string;
  confirmMessage?: string;
  successMessage?: string;
  danger?: boolean;
  run: () => void | Promise<void>;
};

/** Create / edit / save / delete: confirm → call API → result dialog */
export async function runMutatingAction(opts: MutateActionOptions): Promise<boolean> {
  const verb = MUTATE_VERB[opts.action];
  const subject = opts.subject ?? "";
  const confirmed = await showConfirm({
    title: `确认${verb}`,
    message:
      opts.confirmMessage ??
      `确定要${verb}${subject ? `「${subject}」` : ""}吗？此操作将提交到服务端。`,
    okText: `确定${verb}`,
    danger: opts.danger ?? opts.action === "delete",
  });
  if (!confirmed) return false;
  try {
    await opts.run();
    await showResult({
      type: "success",
      title: `${verb}成功`,
      message: opts.successMessage ?? `${subject}${verb}成功`,
    });
    return true;
  } catch (e) {
    await showResult({
      type: "error",
      title: `${verb}失败`,
      message: e instanceof Error ? e.message : String(e),
    });
    return false;
  }
}

export function defaultPaginationState(): PaginationState {
  return { page: 1, pageSize: DEFAULT_PAGE_SIZE };
}

/** Client pagination: table HTML + pager (for innerHTML rendering) */
export function renderPaginatedSection<T>(
  prefix: string,
  items: T[],
  state: PaginationState,
  renderTable: (pageItems: T[]) => string,
  emptyHtml?: string,
): { html: string; page: number } {
  if (!items.length) {
    return { html: emptyHtml ?? emptyState("暂无数据"), page: 1 };
  }
  const page = clampPage(state.page, items.length, state.pageSize);
  const pageItems = paginate(items, page, state.pageSize);
  const html = `${renderTable(pageItems)}${renderPaginationBar(prefix, items.length, page, state.pageSize)}`;
  return { html, page };
}

export type MountPaginatedTableOptions<T> = {
  tableClass?: string;
  wrapClass?: string;
  headHtml: string;
  rowHtml: (item: T, globalIndex: number) => string;
  emptyText?: string;
  onAfterRender?: () => void;
};

/** Client pagination: mount into a container and bind page controls (DOM rendering) */
export function mountPaginatedTable<T>(
  container: HTMLElement,
  prefix: string,
  items: T[],
  options: MountPaginatedTableOptions<T>,
): void {
  let state = defaultPaginationState();
  function paint(): void {
    if (!items.length) {
      container.innerHTML = emptyState(options.emptyText ?? "暂无数据");
      return;
    }
    const page = clampPage(state.page, items.length, state.pageSize);
    state = { ...state, page };
    const pageItems = paginate(items, page, state.pageSize);
    const startIdx = (page - 1) * state.pageSize;
    const rows = pageItems.map((item, i) => options.rowHtml(item, startIdx + i)).join("");
    const tableClass = options.tableClass ?? "data-table";
    const wrapClass = options.wrapClass ?? "ant-table-wrapper";
    container.innerHTML = `
      <div class="${wrapClass}"><table class="${tableClass}">${options.headHtml}<tbody>${rows}</tbody></table></div>
      ${renderPaginationBar(prefix, items.length, page, state.pageSize)}`;
    bindPaginationBar(container, prefix, state, items.length, (next) => {
      state = next;
      paint();
    });
    options.onAfterRender?.();
  }
  paint();
}
