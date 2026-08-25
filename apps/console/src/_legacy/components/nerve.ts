/** Legacy Nerve UI composite wrappers (dialogs, drawers, cards, table rows). */
import {
  btn,
  formField,
  formStack,
  input,
  select,
  tag,
  textarea,
  type BtnOptions,
  type SelectOption,
  type TagTone,
} from "./controls.js";
import { esc, el, htmlAttrs } from "./esc.js";

export type NerveTableColumn = {
  key: string;
  title: string;
  className?: string;
  /** Left-aligned by default; override with nerve-align-center / nerve-align-right */
  align?: "left" | "center" | "right";
};

/** Column alignment modifier classes (left by default; no class required) */
export const NERVE_ALIGN_CENTER = "nerve-align-center";
export const NERVE_ALIGN_RIGHT = "nerve-align-right";

function nerveAlignClass(align?: NerveTableColumn["align"]): string {
  if (align === "center") return NERVE_ALIGN_CENTER;
  if (align === "right") return NERVE_ALIGN_RIGHT;
  return "";
}

export type NerveTableAction = {
  label: string;
  action: string;
  id?: string;
  variant?: BtnOptions["variant"];
  danger?: boolean;
  primary?: boolean;
};

/** Table action-column button group */
export function nerveTableActions(actions: NerveTableAction[]): string {
  const inner = actions
    .map((a) => {
      const variant = a.variant ?? "link";
      const className = [a.danger ? "danger" : "", a.primary ? "nerve-action-primary" : ""]
        .filter(Boolean)
        .join(" ");
      return btn({
        label: a.label,
        variant,
        size: "sm",
        className: className || undefined,
        attrs: {
          "data-nerve-action": a.action,
          ...(a.id ? { "data-id": a.id } : {}),
        },
      });
    })
    .join("");
  return `<div class="nerve-table-actions">${inner}</div>`;
}

/** Standard data table */
export function nerveTable(opts: {
  columns: NerveTableColumn[];
  bodyHtml: string;
  className?: string;
}): string {
  const head = opts.columns
    .map((c) => {
      const cls = [c.className, nerveAlignClass(c.align)].filter(Boolean).join(" ");
      return `<th${cls ? ` class="${cls}"` : ""}>${esc(c.title)}</th>`;
    })
    .join("");
  const cls = ["nerve-table", "data-table", opts.className].filter(Boolean).join(" ");
  return `<div class="nerve-table-wrap"><table class="${cls}"><thead><tr>${head}</tr></thead><tbody>${opts.bodyHtml}</tbody></table></div>`;
}

/** Action-column th/td class names (left by default) */
export const NERVE_COL_ACTIONS = "nerve-col-actions";

/** Table cell class (optional alignment) */
export function nerveTableCell(className: string, align?: NerveTableColumn["align"]): string {
  return [className, nerveAlignClass(align)].filter(Boolean).join(" ");
}

/** Live-threat list container */
export function nerveThreatList(rowsHtml: string): string {
  return `<div class="nerve-threat-list threat-list">${rowsHtml}</div>`;
}

/** Live-threat row (time + tags + IP + URI) */
export function nerveThreatRow(opts: {
  time: string;
  siteLabel: string;
  siteId?: string;
  actionHtml: string;
  categoryLabel: string;
  ip: string;
  uri?: string;
}): string {
  const uri = (opts.uri ?? "").trim();
  return `<div class="nerve-threat-row threat-row">
    <div class="nerve-threat-line">
      <span class="nerve-threat-time threat-time">${esc(opts.time)}</span>
      <div class="nerve-threat-meta">
        <span class="nerve-threat-tags">
          <span class="app-event-tag" title="${esc(opts.siteId ?? "default")}">${esc(opts.siteLabel)}</span>
          ${opts.actionHtml}
          <span class="layer-badge layer-l7 nerve-threat-cat">${esc(opts.categoryLabel)}</span>
        </span>
        <code class="nerve-threat-ip threat-ip">${esc(opts.ip)}</code>
      </div>
    </div>
    ${uri ? `<div class="nerve-threat-detail"><code class="nerve-threat-uri threat-msg muted">${esc(uri)}</code></div>` : ""}
  </div>`;
}

/** Checkbox */
export function nerveCheckbox(opts: {
  id?: string;
  label?: string;
  checked?: boolean;
  value?: string;
  className?: string;
  attrs?: Record<string, string | number | boolean | undefined | null>;
}): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const value = opts.value ? ` value="${esc(opts.value)}"` : "";
  const checked = opts.checked ? " checked" : "";
  const cls = ["nerve-checkbox", opts.className].filter(Boolean).join(" ");
  const inputHtml = `<input type="checkbox" class="${cls}"${id}${value}${checked}${htmlAttrs(opts.attrs)} />`;
  if (!opts.label) return inputHtml;
  return `<label class="nerve-checkbox-wrap">${inputHtml}<span>${esc(opts.label)}</span></label>`;
}

/** Radio */
export function nerveRadio(opts: {
  name: string;
  id?: string;
  label: string;
  value: string;
  checked?: boolean;
  attrs?: Record<string, string | number | boolean | undefined | null>;
}): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const checked = opts.checked ? " checked" : "";
  return `<label class="nerve-radio-wrap">
    <input type="radio" class="nerve-radio" name="${esc(opts.name)}" value="${esc(opts.value)}"${id}${checked}${htmlAttrs(opts.attrs)} />
    <span>${esc(opts.label)}</span>
  </label>`;
}

/** Switch */
export function nerveSwitch(opts: {
  id?: string;
  checked?: boolean;
  label?: string;
  sm?: boolean;
  attrs?: Record<string, string | number | boolean | undefined | null>;
}): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const checked = opts.checked ? " checked" : "";
  const cls = ["nerve-switch", "switch", opts.sm && "switch-sm"].filter(Boolean).join(" ");
  const sw = `<label class="${cls}"><input type="checkbox"${id}${checked}${htmlAttrs(opts.attrs)} /><span></span></label>`;
  if (!opts.label) return sw;
  return `<label class="nerve-switch-wrap switch-wrap">${opts.label ? `<span>${esc(opts.label)}</span>` : ""}${sw}</label>`;
}

/** Section card (rule library / rule-set list blocks) */
export function nerveSectionCard(opts: {
  title: string;
  subtitle?: string;
  count?: string | number;
  countId?: string;
  bodyId?: string;
  className?: string;
}): { open: string; close: string } {
  const bodyId = opts.bodyId ? ` id="${esc(opts.bodyId)}"` : "";
  const cls = ["nerve-section-card", "cr-section", "card", opts.className].filter(Boolean).join(" ");
  const countId = opts.countId ? ` id="${esc(opts.countId)}"` : "";
  const count =
    opts.count != null
      ? `<span class="nerve-section-count cr-section-count"${countId}>${esc(String(opts.count))}</span>`
      : "";
  const open = `<div class="${cls}">
    <div class="nerve-section-head cr-section-head">
      <div><h3>${esc(opts.title)}</h3>${opts.subtitle ? `<p class="muted small">${esc(opts.subtitle)}</p>` : ""}</div>
      ${count}
    </div>
    <div class="nerve-section-body"${bodyId}>`;
  const close = `</div></div>`;
  return { open, close };
}

/** Modal shell (DOM) */
export function mountNerveDialog(opts: {
  title: string;
  wide?: boolean;
  xl?: boolean;
  onClose?: () => void;
}): {
  mask: HTMLElement;
  body: HTMLElement;
  footer: HTMLElement;
  close: () => void;
} {
  const mask = el("div", "nerve-dialog-mask ant-modal-mask");
  const wrap = el("div", "nerve-dialog-wrap ant-modal-wrap");
  const modalCls = [
    "nerve-dialog",
    "ant-modal",
    opts.xl && "ant-modal-xl",
    opts.wide && "ant-modal-wide",
  ]
    .filter(Boolean)
    .join(" ");
  const modal = el("div", modalCls);
  const header = el("div", "nerve-dialog-header ant-modal-header");
  header.innerHTML = `<span>${esc(opts.title)}</span>`;
  const closeBtn = el("button", "nerve-dialog-close ant-modal-close", "×");
  closeBtn.type = "button";
  const close = () => {
    mask.remove();
    opts.onClose?.();
  };
  closeBtn.onclick = close;
  header.appendChild(closeBtn);
  const body = el("div", "nerve-dialog-body ant-modal-body");
  const footer = el("div", "nerve-dialog-footer ant-modal-footer");
  modal.append(header, body, footer);
  modal.addEventListener("click", (e) => e.stopPropagation());
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  document.body.appendChild(mask);
  return { mask, body, footer, close };
}

/** Modal footer button group */
export function nerveDialogFooter(...buttons: string[]): string {
  return buttons.join("");
}

// — aliases matching controls names so pages can import from nerve uniformly —
export {
  btn as nerveBtn,
  formField as nerveFormField,
  formStack as nerveForm,
  input as nerveInput,
  select as nerveSelect,
  tag as nerveTag,
  textarea as nerveTextarea,
  type BtnOptions as NerveBtnOptions,
  type SelectOption as NerveSelectOption,
  type TagTone as NerveTagTone,
};
