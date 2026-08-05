/** Web 核心防护 → CRS 规则页跳转预设（sessionStorage 一次性消费） */

export type CrsViewPreset = {
  category: string;
  protect?: 1 | 2 | 3;
};

const PRESET_KEY = "nerve-crs-view-preset";

/** 防护模块 key → CRS 规则分类 */
export const MODULE_CRS_CATEGORY: Record<string, string | null> = {
  sqli: "sqli",
  xss: "xss",
  rce: "rce",
  path_traversal: "path_traversal",
  ssrf: "ssrf",
  webshell: "webshell",
  file_upload: "webshell",
  info_leak: null,
  bot: null,
  cc: null,
};

export function sensitivityToProtectLevel(s: string): 1 | 2 | 3 {
  if (s === "low") return 1;
  if (s === "high") return 3;
  return 2;
}

export function setCrsViewPreset(preset: CrsViewPreset): void {
  sessionStorage.setItem(PRESET_KEY, JSON.stringify(preset));
}

export function consumeCrsViewPreset(): CrsViewPreset | null {
  const raw = sessionStorage.getItem(PRESET_KEY);
  if (!raw) return null;
  sessionStorage.removeItem(PRESET_KEY);
  try {
    const p = JSON.parse(raw) as CrsViewPreset;
    return p?.category ? p : null;
  } catch {
    return null;
  }
}

export const CRS_NAV_EVENT = "nerve-crs-nav";

export function navigateToCrsRules(category: string, protect?: 1 | 2 | 3): void {
  setCrsViewPreset({ category, protect });
  const onCrs = location.hash.replace(/^#\/?/, "").split("?")[0] === "rules-crs";
  if (onCrs) {
    window.dispatchEvent(new CustomEvent(CRS_NAV_EVENT));
  } else {
    location.hash = "#/rules-crs";
  }
}
