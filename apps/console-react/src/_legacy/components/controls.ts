/** Legacy WAF Console form control HTML wrappers (`.ant-*` / `.form-stack`). */
import { esc, htmlAttrs } from "./esc.js";

export type BtnVariant = "default" | "primary" | "danger" | "link";
export type BtnSize = "md" | "sm";

export type BtnOptions = {
  label: string;
  id?: string;
  variant?: BtnVariant;
  size?: BtnSize;
  type?: "button" | "submit";
  disabled?: boolean;
  className?: string;
  attrs?: Record<string, string | number | boolean | undefined | null>;
};

/** Button: nerve-btn + ant-btn */
export function btn(opts: BtnOptions): string {
  const classs = ["nerve-btn", "ant-btn"];
  const variant = opts.variant ?? "default";
  if (variant === "primary") classs.push("ant-btn-primary");
  if (variant === "danger") classs.push("ant-btn-danger");
  if (variant === "link") classs.push("ant-btn-link");
  if (opts.size === "sm") classs.push("ant-btn-sm");
  if (opts.className) classs.push(opts.className);
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const disabled = opts.disabled ? " disabled" : "";
  return `<button type="${opts.type ?? "button"}" class="${classs.join(" ")}"${id}${disabled}${htmlAttrs(opts.attrs)}>${esc(opts.label)}</button>`;
}

export type InputOptions = {
  id?: string;
  type?: string;
  name?: string;
  value?: string | number;
  placeholder?: string;
  wide?: boolean;
  sm?: boolean;
  className?: string;
  attrs?: Record<string, string | number | boolean | undefined | null>;
};

/** Single-line input */
export function input(opts: InputOptions = {}): string {
  const cls = ["nerve-input", "ant-input", opts.wide && "ant-input-wide", opts.sm && "sm", opts.className]
    .filter(Boolean)
    .join(" ");
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const name = opts.name ? ` name="${esc(opts.name)}"` : "";
  const type = ` type="${esc(opts.type ?? "text")}"`;
  const value = opts.value != null && opts.value !== "" ? ` value="${esc(String(opts.value))}"` : "";
  const placeholder = opts.placeholder ? ` placeholder="${esc(opts.placeholder)}"` : "";
  return `<input class="${cls}"${id}${name}${type}${value}${placeholder}${htmlAttrs(opts.attrs)} />`;
}

export type TextareaOptions = {
  id?: string;
  value?: string;
  placeholder?: string;
  rows?: number;
  wide?: boolean;
  className?: string;
  attrs?: Record<string, string | number | boolean | undefined | null>;
};

/** Multi-line textarea */
export function textarea(opts: TextareaOptions = {}): string {
  const cls = ["nerve-input", "nerve-textarea", "ant-input", opts.wide !== false && "ant-input-wide", opts.className].filter(Boolean).join(" ");
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const rows = opts.rows ? ` rows="${opts.rows}"` : "";
  const placeholder = opts.placeholder ? ` placeholder="${esc(opts.placeholder)}"` : "";
  return `<textarea class="${cls}"${id}${rows}${placeholder}${htmlAttrs(opts.attrs)}>${esc(opts.value ?? "")}</textarea>`;
}

export type SelectOption = { value: string; label: string; selected?: boolean };

export type SelectOptions = {
  id?: string;
  wide?: boolean;
  className?: string;
  options: SelectOption[];
  attrs?: Record<string, string | number | boolean | undefined | null>;
};

/** Select dropdown */
export function select(opts: SelectOptions): string {
  const cls = ["nerve-select", "ant-input", opts.wide && "ant-input-wide", opts.className].filter(Boolean).join(" ");
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const optsHtml = opts.options
    .map(
      (o) =>
        `<option value="${esc(o.value)}"${o.selected ? " selected" : ""}>${esc(o.label)}</option>`,
    )
    .join("");
  return `<select class="${cls}"${id}${htmlAttrs(opts.attrs)}>${optsHtml}</select>`;
}

export type SearchInputOptions = {
  id?: string;
  placeholder?: string;
  value?: string;
  width?: string | number;
  className?: string;
  attrs?: Record<string, string | number | boolean | undefined | null>;
};

/** Search input (for table-toolbar) */
export function searchInput(opts: SearchInputOptions = {}): string {
  const width = opts.width ?? 220;
  const cls = ["nerve-input", "nerve-search", "ant-input", opts.className].filter(Boolean).join(" ");
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const value = opts.value ? ` value="${esc(opts.value)}"` : "";
  return `<input class="${cls}"${id} type="search" placeholder="${esc(opts.placeholder ?? "搜索…")}"${value} style="width:${typeof width === "number" ? `${width}px` : width}"${htmlAttrs(opts.attrs)} />`;
}

export type CardOptions = {
  title: string;
  body: string;
  /** Plain-text subtitle (auto-wrapped in head-tip) */
  tip?: string;
  /** Extra HTML on the title row (may already include head-tip, etc.) */
  tipHtml?: string;
  id?: string;
  bodyId?: string;
  className?: string;
};

/** ant-card panel */
export function card(opts: CardOptions): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const bodyId = opts.bodyId ? ` id="${esc(opts.bodyId)}"` : "";
  const cls = ["nerve-card", "ant-card", opts.className].filter(Boolean).join(" ");
  const tipPart = opts.tipHtml ?? (opts.tip ? `<span class="head-tip">${esc(opts.tip)}</span>` : "");
  return `<div class="${cls}"${id}>
    <div class="ant-card-head">${esc(opts.title)}${tipPart}</div>
    <div class="ant-card-body"${bodyId}>${opts.body}</div>
  </div>`;
}

export type PanelCardOptions = {
  title: string;
  body: string;
  id?: string;
  bodyId?: string;
  className?: string;
};

/** Lightweight panel card (.card; used on rules pages, etc.) */
export function panelCard(opts: PanelCardOptions): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const bodyId = opts.bodyId ? ` id="${esc(opts.bodyId)}"` : "";
  const cls = ["nerve-panel-card", "card", opts.className].filter(Boolean).join(" ");
  return `<div class="${cls}"${id}><h3>${esc(opts.title)}</h3><div${bodyId}>${opts.body}</div></div>`;
}

/** Toolbar above the table */
export function tableToolbar(...parts: (string | false | null | undefined)[]): string {
  return `<div class="nerve-table-toolbar table-toolbar">${parts.filter(Boolean).join("")}</div>`;
}

/** Page-level action button group (hot reload / refresh, etc.) */
export function syncActions(...buttons: (string | false | null | undefined)[]): string {
  return `<div class="nerve-sync-actions sync-actions">${buttons.filter(Boolean).join("")}</div>`;
}

/** Vertical form stack */
export function formStack(content: string, className?: string): string {
  const cls = ["nerve-form", "form-stack", className].filter(Boolean).join(" ");
  return `<div class="${cls}">${content}</div>`;
}

/** Form label */
export function formLabel(text: string): string {
  return `<label class="nerve-form-label ant-form-label">${esc(text)}</label>`;
}

/** Label + control */
export function formField(label: string, control: string): string {
  return `<div class="nerve-form-item ant-form-item">${formLabel(label)}${control}</div>`;
}

/** Inline label + control (toolbar scenarios) */
export function inlineField(label: string, control: string, marginLeft = "16px"): string {
  return `<label style="margin-left:${marginLeft}">${esc(label)} ${control}</label>`;
}

export type AlertTone = "info" | "error" | "success" | "warning";

/** Alert / tip bar */
export function alertBox(tone: AlertTone, message: string, className?: string): string {
  const cls = ["nerve-alert", "ant-alert", `ant-alert-${tone}`, className].filter(Boolean).join(" ");
  return `<div class="${cls}">${message}</div>`;
}

export type TagTone = "default" | "green" | "red" | "orange" | "blue";

/** Status tag */
export function tag(text: string, tone: TagTone = "default"): string {
  const cls = tone === "default" ? "nerve-tag ant-tag" : `nerve-tag ant-tag ant-tag-${tone}`;
  return `<span class="${cls}">${esc(text)}</span>`;
}

export type TabItem = { id: string; label: string; active?: boolean };

/** Tab bar */
export function tabBar(tabs: TabItem[], attr = "data-tab"): string {
  return `<div class="nerve-tab-bar tab-bar">${tabs
    .map(
      (t) =>
        `<button type="button" class="tab-btn${t.active ? " active" : ""}" ${attr}="${esc(t.id)}">${esc(t.label)}</button>`,
    )
    .join("")}</div>`;
}

/** Labeled filter select (common in toolbars) */
export function filterSelect(opts: {
  id: string;
  label?: string;
  width?: number;
  options: SelectOption[];
}): string {
  const width = opts.width ?? 200;
  const label = opts.label ? `${esc(opts.label)} ` : "";
  return `<label${opts.label ? "" : ""}>${label}${select({
    id: opts.id,
    options: opts.options,
    attrs: { style: `width:${width}px;${opts.label ? "margin-left:8px" : ""}` },
  })}</label>`;
}
