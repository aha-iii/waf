/** Display metadata for Web core protection modules (stable order for UI). */
export const MODULE_META: Record<string, { title: string; desc: string }> = {
  sqli: { title: "SQL 注入防护", desc: "检测 UNION SELECT、OR 1=1 等 SQLi 特征" },
  xss: { title: "XSS 防护", desc: "检测 script 标签、事件处理器等跨站脚本" },
  rce: { title: "命令注入防护", desc: "检测 shell 管道、命令拼接" },
  path_traversal: { title: "路径穿越", desc: "检测 ../ 等目录遍历" },
  ssrf: { title: "SSRF 防护", desc: "检测内网地址访问" },
  file_upload: { title: "恶意上传", desc: "限制危险文件类型与内容" },
  webshell: { title: "Webshell", desc: "检测 PHP/JSP 一句话木马特征" },
  info_leak: { title: "信息泄露", desc: "检测响应中的敏感信息（仅记录）" },
  bot: { title: "Bot 管理", desc: "爬虫与自动化工具识别（挑战模式）" },
  cc: { title: "CC 防护", desc: "单 IP 请求频率限制" },
  custom: { title: "自定义规则", desc: "签名规则与自定义插件规则集" },
  plugin: { title: "自定义插件", desc: "L7 Lua 插件" },
  crs: { title: "专家规则", desc: "专家引擎规则包，多层融合检测" },
};

/** Web-attack detection (request signatures / CRS rules; sensitivity and per-rule config) */
export const WEB_ATTACK_MODULE_KEYS = [
  "sqli",
  "xss",
  "rce",
  "path_traversal",
  "ssrf",
  "file_upload",
  "webshell",
] as const;

/** Basic protection: rate limit / Bot (params on dedicated pages) */
export const PRE_ACCESS_MODULE_KEYS = ["cc", "bot"] as const;

/** Response-phase detection */
export const RESPONSE_MODULE_KEYS = ["info_leak"] as const;

/** Sidebar protection-module grid: attack detection + basic protection + response (UI sections) */
export const WEB_CORE_MODULE_KEYS = [
  ...WEB_ATTACK_MODULE_KEYS,
  ...PRE_ACCESS_MODULE_KEYS,
  ...RESPONSE_MODULE_KEYS,
] as const;

export type ModuleConfigKind = "attack" | "pre_access" | "response";

export function moduleConfigKind(key: string): ModuleConfigKind {
  if ((PRE_ACCESS_MODULE_KEYS as readonly string[]).includes(key)) return "pre_access";
  if ((RESPONSE_MODULE_KEYS as readonly string[]).includes(key)) return "response";
  return "attack";
}

/** Console page that holds detailed basic-protection module params */
export const PRE_ACCESS_MODULE_PAGES: Partial<Record<string, string>> = {
  cc: "cc",
  bot: "bot",
};

export const RESPONSE_MODULE_PAGES: Partial<Record<string, string>> = {
  info_leak: "info-leak",
};

/** App protection-module sub-tab: custom rules (signatures and plugins, configured per rule set) */
export const EXTENSION_MODULE_KEYS = ["custom"] as const;

export type ExtensionModuleKey = (typeof EXTENSION_MODULE_KEYS)[number];

export const EXTENSION_MODULE_TABS: { id: ExtensionModuleKey; label: string; globalPage: string }[] = [
  { id: "custom", label: "自定义防护", globalPage: "侧边栏「自定义规则」" },
];

export const MODULE_ORDER = Object.keys(MODULE_META);

const DEFAULT_MODULE_CFG = { enabled: false, action: "block", sensitivity: "medium" };

/** Merge module config in fixed order; fill missing entries with defaults */
export function orderedModuleEntries(
  modules: Record<string, { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }>,
): [string, typeof DEFAULT_MODULE_CFG & { disabled_rules?: string[] }][] {
  return MODULE_ORDER.map((key) => {
    const cfg = modules[key];
    return [
      key,
      cfg
        ? {
            enabled: cfg.enabled,
            action: cfg.action,
            sensitivity: cfg.sensitivity,
            ...(cfg.disabled_rules?.length ? { disabled_rules: cfg.disabled_rules } : {}),
          }
        : { ...DEFAULT_MODULE_CFG },
    ];
  });
}

/** Web core detection modules only (excludes custom / plugin / crs) */
export function orderedWebCoreModuleEntries(
  modules: Record<string, { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }>,
): [string, typeof DEFAULT_MODULE_CFG & { disabled_rules?: string[] }][] {
  return orderedModuleEntries(modules).filter(([key]) =>
    (WEB_CORE_MODULE_KEYS as readonly string[]).includes(key),
  );
}

export function orderedWebAttackModuleEntries(
  modules: Record<string, { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }>,
): [string, typeof DEFAULT_MODULE_CFG & { disabled_rules?: string[] }][] {
  return orderedWebCoreModuleEntries(modules).filter(([key]) =>
    (WEB_ATTACK_MODULE_KEYS as readonly string[]).includes(key),
  );
}

export function orderedPreAccessModuleEntries(
  modules: Record<string, { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }>,
): [string, typeof DEFAULT_MODULE_CFG & { disabled_rules?: string[] }][] {
  return orderedWebCoreModuleEntries(modules).filter(([key]) =>
    (PRE_ACCESS_MODULE_KEYS as readonly string[]).includes(key),
  );
}

export function orderedResponseModuleEntries(
  modules: Record<string, { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }>,
): [string, typeof DEFAULT_MODULE_CFG & { disabled_rules?: string[] }][] {
  return orderedWebCoreModuleEntries(modules).filter(([key]) =>
    (RESPONSE_MODULE_KEYS as readonly string[]).includes(key),
  );
}

export type ModuleCfg = typeof DEFAULT_MODULE_CFG & {
  disabled_rules?: string[];
  requests_per_minute?: number;
  burst?: number;
  block_duration_sec?: number;
  score_threshold?: number;
};

export type ModuleCardEntry = {
  key: string;
  cfg: ModuleCfg;
  meta?: { title: string; desc: string };
};

export const CUSTOM_MODULE_KEY_PREFIX = "custom:";
export const PLUGIN_MODULE_KEY_PREFIX = "plugin:";

export function customModuleKey(setId: string): string {
  return `${CUSTOM_MODULE_KEY_PREFIX}${setId}`;
}

export function pluginModuleKey(name: string): string {
  return `${PLUGIN_MODULE_KEY_PREFIX}${name}`;
}

type ModuleCfgRecord = Record<
  string,
  { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }
>;

function normalizeModuleCfg(
  cfg: {
    enabled: boolean;
    action: string;
    sensitivity: string;
    disabled_rules?: string[];
    requests_per_minute?: number;
    burst?: number;
    block_duration_sec?: number;
    score_threshold?: number;
  } | undefined,
): ModuleCfg {
  if (!cfg) return { ...DEFAULT_MODULE_CFG };
  return {
    enabled: cfg.enabled,
    action: cfg.action,
    sensitivity: cfg.sensitivity,
    ...(cfg.disabled_rules?.length ? { disabled_rules: cfg.disabled_rules } : {}),
    ...(cfg.requests_per_minute !== undefined ? { requests_per_minute: cfg.requests_per_minute } : {}),
    ...(cfg.burst !== undefined ? { burst: cfg.burst } : {}),
    ...(cfg.block_duration_sec !== undefined ? { block_duration_sec: cfg.block_duration_sec } : {}),
    ...(cfg.score_threshold !== undefined ? { score_threshold: cfg.score_threshold } : {}),
  };
}

/** Read module config: prefer exact key; extensions fall back to aggregated custom / plugin */
export function moduleCfgFromRecord(modules: ModuleCfgRecord, key: string): ModuleCfg {
  if (modules[key]) return normalizeModuleCfg(modules[key]);
  if (key.startsWith(CUSTOM_MODULE_KEY_PREFIX) && modules.custom) {
    return normalizeModuleCfg(modules.custom);
  }
  if (key.startsWith(PLUGIN_MODULE_KEY_PREFIX) && modules.plugin) {
    return normalizeModuleCfg(modules.plugin);
  }
  return { ...DEFAULT_MODULE_CFG };
}

export type CustomRuleSetInfo = {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
};

export type PluginDescriptorInfo = {
  name: string;
  version: string;
  description: string;
  layer: string;
  enabled: boolean;
};

/** One card per custom rule set.
 * `onlyMounted`: only sets already present as `custom:{id}` in modules (strategy snapshot mounts),
 * so the full signature rule library is not shown as "enabled for this app".
 */
export function buildCustomModuleEntries(
  modules: ModuleCfgRecord,
  sets: CustomRuleSetInfo[],
  opts?: { onlyMounted?: boolean },
): ModuleCardEntry[] {
  const mountedIds = Object.keys(modules)
    .filter((k) => k.startsWith(CUSTOM_MODULE_KEY_PREFIX))
    .map((k) => k.slice(CUSTOM_MODULE_KEY_PREFIX.length));
  const byId = new Map(sets.map((s) => [s.id, s]));

  const ids = opts?.onlyMounted
    ? mountedIds
    : [...new Set([...sets.map((s) => s.id), ...mountedIds])];

  return ids.map((id) => {
    const set = byId.get(id) ?? {
      id,
      name: id,
      description: "",
      enabled: true,
    };
    const key = customModuleKey(id);
    const hasOwn = Boolean(modules[key]);
    const cfg = moduleCfgFromRecord(modules, key);
    if (!hasOwn && !modules.custom) {
      cfg.enabled = set.enabled;
    }
    return {
      key,
      cfg,
      meta: {
        title: set.name || set.id,
        desc: set.description?.trim() || "Console 配置的 custom 规则集",
      },
    };
  });
}

/** Each L7 custom plugin (legacy key plugin:name; new rules use the custom rules page) */
export function buildPluginModuleEntries(
  modules: ModuleCfgRecord,
  plugins: PluginDescriptorInfo[],
): ModuleCardEntry[] {
  return plugins
    .filter((p) => p.layer.toLowerCase() === "l7")
    .map((p) => {
      const key = pluginModuleKey(p.name);
      const hasOwn = Boolean(modules[key]);
      const cfg = moduleCfgFromRecord(modules, key);
      if (!hasOwn && !modules.plugin) {
        cfg.enabled = p.enabled;
      }
      return {
        key,
        cfg,
        meta: {
          title: p.name,
          desc: p.description?.trim() || `L7 插件 v${p.version}`,
        },
      };
    });
}
