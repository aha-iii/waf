/**
 * WAF Console 基础控件 HTML 封装
 * 样式类名与 style.css 中 `.ant-*` / `.form-stack` 等保持一致
 */
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

/** 按钮：nerve-btn + ant-btn */
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

/** 单行输入框 */
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

/** 多行输入框 */
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

/** 下拉框 */
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

/** 搜索框（用于 table-toolbar） */
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
  /** 纯文本副标题（自动包 head-tip） */
  tip?: string;
  /** 标题行附加 HTML（已含 head-tip 等） */
  tipHtml?: string;
  id?: string;
  bodyId?: string;
  className?: string;
};

/** ant-card 卡片 */
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

/** 轻量 panel 卡片（.card，用于规则页等） */
export function panelCard(opts: PanelCardOptions): string {
  const id = opts.id ? ` id="${esc(opts.id)}"` : "";
  const bodyId = opts.bodyId ? ` id="${esc(opts.bodyId)}"` : "";
  const cls = ["nerve-panel-card", "card", opts.className].filter(Boolean).join(" ");
  return `<div class="${cls}"${id}><h3>${esc(opts.title)}</h3><div${bodyId}>${opts.body}</div></div>`;
}

/** 表格上方工具栏 */
export function tableToolbar(...parts: (string | false | null | undefined)[]): string {
  return `<div class="nerve-table-toolbar table-toolbar">${parts.filter(Boolean).join("")}</div>`;
}

/** 页面级操作按钮组（热加载 / 刷新等） */
export function syncActions(...buttons: (string | false | null | undefined)[]): string {
  return `<div class="nerve-sync-actions sync-actions">${buttons.filter(Boolean).join("")}</div>`;
}

/** 垂直表单容器 */
export function formStack(content: string, className?: string): string {
  const cls = ["nerve-form", "form-stack", className].filter(Boolean).join(" ");
  return `<div class="${cls}">${content}</div>`;
}

/** 表单标签 */
export function formLabel(text: string): string {
  return `<label class="nerve-form-label ant-form-label">${esc(text)}</label>`;
}

/** 标签 + 控件 */
export function formField(label: string, control: string): string {
  return `<div class="nerve-form-item ant-form-item">${formLabel(label)}${control}</div>`;
}

/** 内联标签 + 控件（toolbar 场景） */
export function inlineField(label: string, control: string, marginLeft = "16px"): string {
  return `<label style="margin-left:${marginLeft}">${esc(label)} ${control}</label>`;
}

export type AlertTone = "info" | "error" | "success" | "warning";

/** 提示条 */
export function alertBox(tone: AlertTone, message: string, className?: string): string {
  const cls = ["nerve-alert", "ant-alert", `ant-alert-${tone}`, className].filter(Boolean).join(" ");
  return `<div class="${cls}">${message}</div>`;
}

export type TagTone = "default" | "green" | "red" | "orange" | "blue";

/** 状态标签 */
export function tag(text: string, tone: TagTone = "default"): string {
  const cls = tone === "default" ? "nerve-tag ant-tag" : `nerve-tag ant-tag ant-tag-${tone}`;
  return `<span class="${cls}">${esc(text)}</span>`;
}

export type TabItem = { id: string; label: string; active?: boolean };

/** Tab 切换条 */
export function tabBar(tabs: TabItem[], attr = "data-tab"): string {
  return `<div class="nerve-tab-bar tab-bar">${tabs
    .map(
      (t) =>
        `<button type="button" class="tab-btn${t.active ? " active" : ""}" ${attr}="${esc(t.id)}">${esc(t.label)}</button>`,
    )
    .join("")}</div>`;
}

/** 带 label 的下拉筛选（toolbar 常用） */
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
