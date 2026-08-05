import { MODULE_META } from "../module-meta";
import type { ModuleConfig, ProtectionModules } from "./waf-types";

/** 卡片上显示的模块短名 */
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

const CAP_MODULE_ORDER = [
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

export function orderedCapabilityKeys(modules: Record<string, ModuleConfig>): string[] {
  return [
    ...CAP_MODULE_ORDER.filter((k) => modules[k]),
    ...Object.keys(modules).filter((k) => !CAP_MODULE_ORDER.includes(k)),
  ];
}

export function moduleTagSuffix(cfg: ModuleConfig, globalMode: string): string {
  if (!cfg.enabled) return "";
  if (globalMode === "detect" || cfg.action === "log") return "·检";
  if (cfg.action === "challenge") return "·验";
  return "";
}

export type ProtectionModulesLike = Pick<ProtectionModules, "mode" | "modules">;
