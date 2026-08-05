/** 语义配置类型与合并（自原 semantic-config-ui 抽出，无 DOM） */

import {
  defaultPipelineEditor,
  mergePipelineEditor,
  type PipelineEditorState,
} from "./pipeline-flow";

export type { PipelineEditorState };

export type DecodePolicy = {
  max_depth: number;
  max_expand_bytes: number;
  url: boolean;
  html_entity: boolean;
  base64: boolean;
  hex: boolean;
  unicode: boolean;
  chr: boolean;
  order: string[];
};

export type DetectionScope = {
  query: boolean;
  body: boolean;
  header: boolean;
  cookie: boolean;
  path: boolean;
};

export type SemanticWhitelist = {
  param_names: string[];
  path_prefixes: string[];
  content_types: string[];
};

export type EngineFusion = {
  semantic_first: boolean;
  libinjection: boolean;
  sql_intent: boolean;
  fallback_plain_libinjection: boolean;
  pipeline_order: string[];
  pipeline_enabled?: Record<string, boolean>;
};

export type SemanticPerformance = {
  max_slices_per_request: number;
  slow_path_sample_rate: number;
  timeout_ms: number;
};

export type SemanticAutoTune = {
  enabled: boolean;
  false_positive_threshold: number;
  auto_whitelist_param: boolean;
  auto_relax_mode: boolean;
  relax_after_fp_count: number;
  window_hours: number;
};

export type AiProtectionConfig = {
  enabled: boolean;
};

export type SemanticConfig = {
  enabled: boolean;
  mode: "strict" | "standard" | "relaxed";
  decode: DecodePolicy;
  scope: DetectionScope;
  whitelist: SemanticWhitelist;
  engines: EngineFusion;
  performance: SemanticPerformance;
  auto_tune: SemanticAutoTune;
  intent_enabled: boolean;
  /** AI 防护：只观测不拦截 */
  ai_protection?: AiProtectionConfig;
  /** 应用侧省略/undefined = 继承全局 */
  expert_level?: "low" | "medium" | "high";
  pipeline_editor?: PipelineEditorState;
};

export type SemanticStats = {
  site_id?: string;
  backend?: string;
  counters: Record<string, number>;
  hit_rate_pct: number;
  recent_samples: {
    ts: string;
    param_path: string;
    decode_chain: string[];
    semantic_score: number;
    engines: string[];
    uri: string;
    hit: boolean;
  }[];
  feedback: {
    ts: string;
    event_id?: string;
    verdict: string;
    note?: string;
    applied_action?: string;
  }[];
};

export type SemanticTab =
  | "general"
  | "decode"
  | "scope"
  | "whitelist"
  | "engines"
  | "performance"
  | "autotune"
  | "observe";

export const DECODE_OPS = ["url", "html_entity", "base64", "hex", "unicode", "chr"] as const;

export const STRATEGY_SEMANTIC_TABS: SemanticTab[] = [
  "general",
  "decode",
  "scope",
  "whitelist",
  "engines",
];

export const SEMANTIC_NAV_ITEMS: { id: SemanticTab; icon: string; label: string; desc: string }[] = [
  { id: "general", icon: "⚙", label: "基础设置", desc: "站点 · 总开关 · 语义模式" },
  { id: "decode", icon: "🔓", label: "解码策略", desc: "编码开关 · 顺序 · 递归深度" },
  { id: "scope", icon: "🎯", label: "检测范围", desc: "Query / Body / Header 等" },
  { id: "whitelist", icon: "✓", label: "业务白名单", desc: "参数 · 路径 · Content-Type" },
  { id: "engines", icon: "🔀", label: "引擎组合", desc: "语义 · 流水线顺序" },
  { id: "performance", icon: "⏱", label: "性能", desc: "切片上限 · 超时 · 采样" },
  { id: "autotune", icon: "🔄", label: "自动调优", desc: "误报反馈闭环" },
  { id: "observe", icon: "📊", label: "观测", desc: "命中率 · 解码链 · 反馈" },
];

export function defaultSemanticConfig(): SemanticConfig {
  return {
    enabled: true,
    mode: "standard",
    decode: {
      max_depth: 8,
      max_expand_bytes: 65536,
      url: true,
      html_entity: true,
      base64: true,
      hex: false,
      unicode: false,
      chr: false,
      order: [...DECODE_OPS],
    },
    scope: { query: true, body: true, header: true, cookie: true, path: true },
    whitelist: { param_names: [], path_prefixes: [], content_types: [] },
    engines: {
      semantic_first: true,
      libinjection: true,
      sql_intent: true,
      fallback_plain_libinjection: true,
      pipeline_order: ["pre_access", "expert", "core", "custom", "ai"],
      pipeline_enabled: { ai: false },
    },
    ai_protection: { enabled: false },
    performance: {
      max_slices_per_request: 128,
      slow_path_sample_rate: 100,
      timeout_ms: 50,
    },
    auto_tune: {
      enabled: true,
      false_positive_threshold: 3,
      auto_whitelist_param: true,
      auto_relax_mode: true,
      relax_after_fp_count: 10,
      window_hours: 168,
    },
    intent_enabled: true,
    pipeline_editor: defaultPipelineEditor(),
  };
}

export function mergeSemanticConfig(raw: Partial<SemanticConfig> | undefined): SemanticConfig {
  const d = defaultSemanticConfig();
  if (!raw) return d;
  let enabled = raw.enabled ?? d.enabled;
  if (raw.engines?.pipeline_enabled && "semantic" in raw.engines.pipeline_enabled
      && raw.engines.pipeline_enabled.semantic === false) {
    enabled = false;
  }
  const orderRaw = raw.engines?.pipeline_order ?? d.engines.pipeline_order;
  return {
    ...d,
    ...raw,
    enabled,
    decode: { ...d.decode, ...raw.decode },
    scope: { ...d.scope, ...raw.scope },
    whitelist: { ...d.whitelist, ...raw.whitelist },
    engines: {
      ...d.engines,
      ...raw.engines,
      pipeline_order: orderRaw.filter((id) => id !== "semantic" && id !== "plugin"),
    },
    performance: { ...d.performance, ...raw.performance },
    auto_tune: { ...d.auto_tune, ...raw.auto_tune },
    ai_protection: {
      enabled: raw.ai_protection?.enabled ?? d.ai_protection?.enabled ?? false,
      ...d.ai_protection,
      ...raw.ai_protection,
    },
    pipeline_editor: mergePipelineEditor(raw.pipeline_editor ?? d.pipeline_editor),
  };
}

export function semanticConfigSnapshot(cfg: SemanticConfig): string {
  return JSON.stringify({
    enabled: cfg.enabled,
    mode: cfg.mode,
    decode: cfg.decode,
    scope: cfg.scope,
    whitelist: cfg.whitelist,
    engines: {
      semantic_first: cfg.engines.semantic_first,
      libinjection: cfg.engines.libinjection,
      sql_intent: cfg.engines.sql_intent,
      fallback_plain_libinjection: cfg.engines.fallback_plain_libinjection,
      pipeline_order: cfg.engines.pipeline_order,
    },
    performance: cfg.performance,
    auto_tune: cfg.auto_tune,
    intent_enabled: cfg.intent_enabled,
    expert_level: cfg.expert_level ?? null,
  });
}

export function linesToList(s: string): string[] {
  return s
    .split("\n")
    .map((x) => x.trim())
    .filter(Boolean);
}

export function listToLines(arr: string[] | undefined): string {
  return (arr ?? []).join("\n");
}

/** 策略模板语义配置：全局默认 + 模板覆盖 */
export function semanticFromStrategyTemplate(
  template: { semantic_mode?: string; semantic?: Record<string, unknown> },
  globalCfg: SemanticConfig,
): SemanticConfig {
  const raw = (template.semantic ?? {}) as Partial<SemanticConfig>;
  const mode = (template.semantic_mode || raw.mode || globalCfg.mode) as SemanticConfig["mode"];
  return mergeSemanticConfig({
    ...globalCfg,
    ...raw,
    mode,
    enabled: raw.enabled ?? globalCfg.enabled,
    intent_enabled: raw.intent_enabled ?? globalCfg.intent_enabled,
    decode: { ...globalCfg.decode, ...raw.decode },
    scope: { ...globalCfg.scope, ...raw.scope },
    whitelist: { ...globalCfg.whitelist, ...raw.whitelist },
    engines: { ...globalCfg.engines, ...raw.engines },
    performance: { ...globalCfg.performance, ...raw.performance },
    auto_tune: { ...globalCfg.auto_tune, ...raw.auto_tune },
  });
}

/** 写入策略库 semantic 字段 */
export function semanticToStrategyJson(cfg: SemanticConfig): Record<string, unknown> {
  return {
    enabled: cfg.enabled,
    mode: cfg.mode,
    decode: cfg.decode,
    scope: cfg.scope,
    whitelist: cfg.whitelist,
    engines: cfg.engines,
    performance: cfg.performance,
    auto_tune: cfg.auto_tune,
    intent_enabled: cfg.intent_enabled,
  };
}

