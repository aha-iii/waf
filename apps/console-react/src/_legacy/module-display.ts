/** Legacy (pre-React) console module: Module display. */
import { MODULE_META } from "./module-meta.js";

export type ModuleConfig = {
  enabled: boolean;
  action: string;
  sensitivity: string;
};

export type ProtectionModulesLike = {
  mode: string;
  modules: Record<string, ModuleConfig>;
};

/** Short module name shown on cards */
export const MODULE_SHORT: Record<string, string> = {
  sqli: "SQLi",
  xss: "XSS",
  rce: "RCE",
  path_traversal: "路径",
  ssrf: "SSRF",
  file_upload: "上传",
  webshell: "Shell",
  info_leak: "泄露",
  bot: "Bot",
  cc: "CC",
  custom: "自定义",
  plugin: "插件",
  crs: "专家",
};

const MODULE_ORDER = [
  "custom",
  "sqli",
  "xss",
  "rce",
  "path_traversal",
  "ssrf",
  "webshell",
  "file_upload",
  "crs",
  "plugin",
  "bot",
  "cc",
  "info_leak",
];

export function moduleShortLabel(key: string): string {
  return MODULE_SHORT[key] ?? key;
}

export function moduleBadgeClasses(cfg: ModuleConfig, globalMode: string): string {
  if (!cfg.enabled) return "mod-state-badge mod-badge-off";
  const sens = cfg.sensitivity === "low" || cfg.sensitivity === "high" ? cfg.sensitivity : "medium";
  const detect = globalMode === "detect" || cfg.action === "log";
  return `mod-state-badge mod-badge-on mod-badge-${sens}${detect ? " mod-badge-detect" : ""}`;
}

export function moduleBadgeLabel(cfg: ModuleConfig): string {
  if (!cfg.enabled) return "未启用";
  if (cfg.sensitivity === "low") return "低灵敏度";
  if (cfg.sensitivity === "high") return "高灵敏度";
  return "中灵敏度";
}

export function moduleTagClasses(cfg: ModuleConfig, globalMode: string): string {
  if (!cfg.enabled) return "mod-tag mod-tag-off";
  const sens = cfg.sensitivity === "low" || cfg.sensitivity === "high" ? cfg.sensitivity : "medium";
  const detect = globalMode === "detect" || cfg.action === "log";
  return `mod-tag mod-tag-on mod-tag-${sens}${detect ? " mod-tag-detect" : ""}`;
}

export function moduleTagTitle(key: string, cfg: ModuleConfig, globalMode: string): string {
  const meta = MODULE_META[key]?.title ?? key;
  if (!cfg.enabled) return `${meta}：未启用`;
  const sens = cfg.sensitivity === "low" ? "低" : cfg.sensitivity === "high" ? "高" : "中";
  const act =
    globalMode === "detect"
      ? "检测模式（仅记录）"
      : cfg.action === "log"
        ? "仅记录"
        : cfg.action === "challenge"
          ? "挑战"
          : "拦截";
  return `${meta}：${act} · 灵敏度${sens}`;
}

export function renderModuleTag(key: string, cfg: ModuleConfig, globalMode: string, esc: (s: string) => string): string {
  const label = moduleShortLabel(key);
  const suffix =
    cfg.enabled && (globalMode === "detect" || cfg.action === "log")
      ? "·检"
      : cfg.enabled && cfg.action === "challenge"
        ? "·验"
        : "";
  return `<span class="${moduleTagClasses(cfg, globalMode)}" title="${esc(moduleTagTitle(key, cfg, globalMode))}">${esc(label)}${suffix}</span>`;
}

/** App card: color module tags only (no global block/detect tags) */
export function renderSiteCapabilityTags(
  modules: ProtectionModulesLike,
  esc: (s: string) => string,
): string {
  const keys = [
    ...MODULE_ORDER.filter((k) => modules.modules[k]),
    ...Object.keys(modules.modules).filter((k) => !MODULE_ORDER.includes(k)),
  ];

  return keys
    .map((key) => {
      const cfg = modules.modules[key];
      if (!cfg) return "";
      return renderModuleTag(key, cfg, modules.mode, esc);
    })
    .join("");
}

export function bindModuleCardStyles(container: HTMLElement): void {
  const modeSel =
    (container.querySelector("#app-mod-mode") as HTMLSelectElement | null) ??
    (container.querySelector("#mod-mode") as HTMLSelectElement | null);

  const refresh = (card: HTMLElement) => {
    const enabled = (card.querySelector(".mod-enabled") as HTMLInputElement)?.checked ?? false;
    card.classList.toggle("mod-card-disabled", !enabled);
  };

  container.querySelectorAll<HTMLElement>(".app-mod-card").forEach((card) => {
    refresh(card);
    card.querySelectorAll(".mod-enabled, .mod-action, .mod-sensitivity").forEach((inp) => {
      inp.addEventListener("change", () => refresh(card));
    });
  });

  const onModeChange = () => {
    container.querySelectorAll<HTMLElement>(".app-mod-card").forEach(refresh);
  };
  modeSel?.addEventListener("change", onModeChange);
}
