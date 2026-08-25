/** DOM / HTML escape helpers for legacy views. */
export function esc(s: string | null | undefined): string {
  return String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function el(tag: string, cls?: string, html?: string): HTMLElement {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (html != null) n.innerHTML = html;
  return n;
}

/** 拼接 HTML 属性（跳过 null / undefined / false） */
export function htmlAttrs(attrs?: Record<string, string | number | boolean | undefined | null>): string {
  if (!attrs) return "";
  return Object.entries(attrs)
    .filter(([, v]) => v != null && v !== false)
    .map(([k, v]) => ` ${k}="${esc(String(v))}"`)
    .join("");
}
