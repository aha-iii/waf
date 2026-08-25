/** Shared library helpers: Strategy. */
import {
  CUSTOM_MODULE_KEY_PREFIX,
  customModuleKey,
} from "../module-meta";

import {
  PRE_ACCESS_MODULE_KEYS,
  RESPONSE_MODULE_KEYS,
} from "../module-meta";

export type StrategyTemplate = {
  id: string;
  name: string;
  description: string;
  semantic_mode: string;
  expert_level: string;
  builtin: boolean;
  sort_order: number;
  site_id?: string | null;
  scope?: "global" | "app";
  enabled?: boolean;
  expert?: { level?: string };
  semantic?: Record<string, unknown>;
  core?: {
    version?: number;
    mode?: string;
    modules?: Record<
      string,
      {
        enabled: boolean;
        action: string;
        sensitivity: string;
        disabled_rules?: string[];
        requests_per_minute?: number;
        burst?: number;
        block_duration_sec?: number;
        score_threshold?: number;
      }
    >;
  };
  custom?: Record<string, unknown>;
};

export type StrategyModuleCfg = {
  enabled: boolean;
  action: string;
  sensitivity: string;
  disabled_rules?: string[];
  requests_per_minute?: number;
  burst?: number;
  block_duration_sec?: number;
  score_threshold?: number;
};

export const SEMANTIC_OPTS = [
  { value: "relaxed", label: "宽松", desc: "更少误报" },
  { value: "standard", label: "标准", desc: "推荐默认" },
  { value: "strict", label: "严格", desc: "更严拦截" },
] as const;

export const EXPERT_OPTS = [
  { value: "low", label: "低", desc: "高置信优先" },
  { value: "medium", label: "中", desc: "推荐默认" },
  { value: "high", label: "高", desc: "更全面覆盖" },
] as const;

export function strategyScopeLabel(t: StrategyTemplate): string {
  if (t.site_id || t.scope === "app") return "本应用";
  if (t.builtin) return "内置";
  return "全局";
}

export function isAppPrivateStrategy(t: StrategyTemplate | null | undefined): boolean {
  return !!(t?.site_id && String(t.site_id).trim());
}

export function isStrategyEnabled(t: StrategyTemplate | null | undefined): boolean {
  return t?.enabled !== false;
}

export function semanticModeLabel(v: string): string {
  return SEMANTIC_OPTS.find((o) => o.value === v)?.label ?? v;
}

export function expertLevelLabel(v: string): string {
  return EXPERT_OPTS.find((o) => o.value === v)?.label ?? v;
}

/** Basic protection modules (CC / Bot / info-leak) enablement summary */
export function strategyPreAccessSummary(tpl: StrategyTemplate): string {
  const mods = tpl.core?.modules ?? {};
  const keys = [...PRE_ACCESS_MODULE_KEYS, ...RESPONSE_MODULE_KEYS] as string[];
  const enabled = keys.filter((k) => mods[k]?.enabled).length;
  if (enabled === 0) return "未启用";
  return `${enabled} 项已启用`;
}

/** Core attack-detection modules enablement summary */
export function strategyCoreAttackSummary(tpl: StrategyTemplate): string {
  const mods = tpl.core?.modules ?? {};
  const attackKeys = [
    "sqli",
    "xss",
    "rce",
    "path_traversal",
    "ssrf",
    "file_upload",
    "webshell",
  ];
  const enabled = attackKeys.filter((k) => mods[k]?.enabled).length;
  if (enabled === 0) return "未启用";
  return `${enabled} 类已启用`;
}

export function toneForSemanticMode(mode: string): "green" | "blue" | "orange" | "default" {
  if (mode === "relaxed") return "green";
  if (mode === "strict") return "orange";
  if (mode === "standard") return "blue";
  return "default";
}

export function toneForExpertLevel(level: string): "green" | "blue" | "orange" | "default" {
  if (level === "low") return "blue";
  if (level === "high") return "orange";
  if (level === "medium") return "blue";
  return "default";
}

export function toneForPreAccessSummary(summary: string): "green" | "default" {
  return summary === "未启用" ? "default" : "green";
}

export function toneForCoreSummary(summary: string): "orange" | "default" {
  return summary === "未启用" ? "default" : "orange";
}

export function toneForCustomSummary(mounted: boolean): "green" | "default" {
  return mounted ? "green" : "default";
}

export function strategyEnabledLabel(enabled: boolean): string {
  return enabled ? "启用" : "停用";
}

export function strategyEnabledTone(enabled: boolean): "green" | "default" {
  return enabled ? "green" : "default";
}

function strategyPipelineStageEnabled(tpl: StrategyTemplate, stage: string): boolean {
  const sem = tpl.semantic as { engines?: { pipeline_enabled?: Record<string, boolean> } } | undefined;
  const map = sem?.engines?.pipeline_enabled;
  if (map && Object.prototype.hasOwnProperty.call(map, stage)) {
    return map[stage] !== false;
  }
  return true;
}

/** Strategy semantic / NSP master switch */
export function strategySemanticEnabled(tpl: StrategyTemplate): boolean {
  const sem = tpl.semantic;
  if (sem && typeof sem.enabled === "boolean") return sem.enabled;
  return true;
}

export function strategyPreAccessEnabled(tpl: StrategyTemplate): boolean {
  const mods = tpl.core?.modules ?? {};
  const keys = [...PRE_ACCESS_MODULE_KEYS, ...RESPONSE_MODULE_KEYS] as string[];
  return keys.some((k) => mods[k]?.enabled);
}

export function strategyExpertEnabled(tpl: StrategyTemplate): boolean {
  return strategyPipelineStageEnabled(tpl, "expert");
}

export function strategyCoreEnabled(tpl: StrategyTemplate): boolean {
  const mods = tpl.core?.modules ?? {};
  const attackKeys = [
    "sqli",
    "xss",
    "rce",
    "path_traversal",
    "ssrf",
    "file_upload",
    "webshell",
  ];
  return attackKeys.some((k) => mods[k]?.enabled);
}

export function strategyCustomEnabled(tpl: StrategyTemplate): boolean {
  return strategyCustomSetIds(tpl).length > 0;
}

/** Custom rule-set IDs mounted on the strategy (prefer custom.set_ids; fall back to custom: keys in core.modules) */
export function strategyCustomSetIds(tpl: StrategyTemplate): string[] {
  const raw = tpl.custom as { set_ids?: unknown } | undefined;
  if (Array.isArray(raw?.set_ids) && raw.set_ids.length) {
    return [...new Set(raw.set_ids.map((x) => String(x).trim()).filter(Boolean))];
  }
  const mods = tpl.core?.modules ?? {};
  return Object.keys(mods)
    .filter((k) => k.startsWith(CUSTOM_MODULE_KEY_PREFIX))
    .map((k) => k.slice(CUSTOM_MODULE_KEY_PREFIX.length));
}

/** Sync set_ids into core.modules custom:{id} keys (submit together when saving custom detection) */
export function buildCoreWithCustomSets(
  core: StrategyTemplate["core"] | undefined,
  setIds: string[],
): NonNullable<StrategyTemplate["core"]> {
  const base = core ?? { version: 1, mode: "block", modules: {} };
  const modules: Record<string, StrategyModuleCfg> = { ...(base.modules ?? {}) };
  for (const key of Object.keys(modules)) {
    if (key.startsWith(CUSTOM_MODULE_KEY_PREFIX) || key === "custom") {
      delete modules[key];
    }
  }
  for (const id of setIds) {
    const key = customModuleKey(id);
    modules[key] = modules[key] ?? {
      enabled: true,
      action: "block",
      sensitivity: "medium",
    };
    modules[key]!.enabled = true;
  }
  return {
    version: base.version ?? 1,
    mode: base.mode ?? "block",
    modules,
  };
}
