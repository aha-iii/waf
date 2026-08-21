/** Console sidebar navigation definitions and hash routes. */
export type PageId =
  | "dashboard"
  | "assets"
  | "events"
  | "basic-protection"
  | "core-protection"
  | "modules"
  | "protection-levels"
  | "access"
  | "cc"
  | "geo"
  | "bot"
  | "info-leak"
  | "intel"
  | "alerts"
  | "analytics"
  | "traffic-dashboard"
  | "rules-crs"
  | "rules-ai"
  | "rules-custom"
  | "rules-plugins-json"
  | "rules-plugins-lua"
  | "ai-semantic"
  | "ai-triage"
  | "ai-posture"
  | "ai-policy-learning"
  | "advanced"
  | "users"
  | "origin-upstreams";

export type NavItem = {
  id: PageId;
  label: string;
  icon: string;
  group: string;
  /** Collapsible group title (e.g. basic protection) with sibling sub-pages beneath */
  subgroup?: string;
  /** @deprecated Children render as siblings of the group title; nested is unused */
  nested?: boolean;
  desc?: string;
};

/** Console build id — sidebar footer should show this version */
export const CONSOLE_BUILD = "2026.07.23-custom-protection-plugins";

export const PROTECTION_SETTINGS_GROUP = "防护设置";

const BASIC_PROTECTION_MENU = "基础防护";
const CORE_PROTECTION_MENU = "核心防护";
const ACCESS_CONTROL_MENU = "访问控制";
const CUSTOM_PROTECTION_MENU = "自定义防护";

export const NAV: NavItem[] = [
  { id: "dashboard", label: "WAF 总览", icon: "◉", group: "总览" },
  { id: "assets", label: "防护站点", icon: "⌂", group: "总览" },

  {
    id: "origin-upstreams",
    label: "Upstreams",
    icon: "⬡",
    group: "源站配置",
    subgroup: "源站配置",
  },

  { id: "protection-levels", label: "防护策略", icon: "▣", group: PROTECTION_SETTINGS_GROUP },

  {
    id: "basic-protection",
    label: "基础防护",
    icon: "◆",
    group: PROTECTION_SETTINGS_GROUP,
    subgroup: BASIC_PROTECTION_MENU,
  },
  { id: "cc", label: "CC 防护", icon: "⏱", group: PROTECTION_SETTINGS_GROUP, subgroup: BASIC_PROTECTION_MENU },
  { id: "bot", label: "Bot 管理", icon: "🤖", group: PROTECTION_SETTINGS_GROUP, subgroup: BASIC_PROTECTION_MENU },
  { id: "info-leak", label: "泄露", icon: "🔒", group: PROTECTION_SETTINGS_GROUP, subgroup: BASIC_PROTECTION_MENU },

  {
    id: "core-protection",
    label: "核心防护",
    icon: "🛡",
    group: PROTECTION_SETTINGS_GROUP,
    subgroup: CORE_PROTECTION_MENU,
  },
  {
    id: "rules-crs",
    label: "核心规则库",
    icon: "📋",
    group: PROTECTION_SETTINGS_GROUP,
    subgroup: CORE_PROTECTION_MENU,
  },

  { id: "rules-custom", label: "签名规则", icon: "✎", group: PROTECTION_SETTINGS_GROUP, subgroup: CUSTOM_PROTECTION_MENU },
  { id: "rules-plugins-lua", label: "Lua 插件", icon: "☾", group: PROTECTION_SETTINGS_GROUP, subgroup: CUSTOM_PROTECTION_MENU },

  { id: "ai-semantic", label: "语义设置", icon: "🔤", group: PROTECTION_SETTINGS_GROUP },
  { id: "rules-ai", label: "AI 防护", icon: "🧠", group: PROTECTION_SETTINGS_GROUP },

  { id: "geo", label: "地域封禁", icon: "🌐", group: PROTECTION_SETTINGS_GROUP, subgroup: ACCESS_CONTROL_MENU },
  { id: "access", label: "IP 访问控制", icon: "⊘", group: PROTECTION_SETTINGS_GROUP, subgroup: ACCESS_CONTROL_MENU },

  { id: "ai-triage", label: "智能研判", icon: "🧪", group: "研判与解释" },
  { id: "ai-posture", label: "防护巡检", icon: "🔍", group: "研判与解释" },
  { id: "ai-policy-learning", label: "策略与学习", icon: "📈", group: "研判与解释" },

  { id: "events", label: "攻击日志", icon: "▤", group: "安全运营" },
  { id: "intel", label: "威胁情报", icon: "⚡", group: "安全运营" },
  { id: "alerts", label: "告警配置", icon: "⚠", group: "安全运营" },
  { id: "traffic-dashboard", label: "攻击大盘", icon: "◐", group: "安全运营" },
  { id: "analytics", label: "WAF 报表", icon: "📊", group: "安全运营" },

  { id: "users", label: "用户管理", icon: "👤", group: "系统" },
  { id: "advanced", label: "高级安全", icon: "⚙", group: "系统" },
];

export const PAGE_META: Record<PageId, { title: string; subtitle: string }> = {
  dashboard: { title: "WAF 总览", subtitle: "" },
  assets: { title: "防护站点", subtitle: "" },
  "origin-upstreams": { title: "Upstreams", subtitle: "" },
  events: { title: "攻击日志", subtitle: "含事件详情 · 攻击路径图谱" },
  "basic-protection": {
    title: "基础防护",
    subtitle: "",
  },
  "core-protection": {
    title: "核心防护",
    subtitle: "",
  },
  modules: {
    title: "核心防护",
    subtitle: "",
  },
  "protection-levels": {
    title: "防护策略",
    subtitle: "",
  },
  access: { title: "IP 访问控制", subtitle: "" },
  cc: { title: "CC 防护", subtitle: "" },
  geo: { title: "地域封禁", subtitle: "" },
  bot: { title: "Bot 管理", subtitle: "" },
  "info-leak": {
    title: "泄露",
    subtitle: "",
  },
  intel: { title: "威胁情报", subtitle: "" },
  alerts: { title: "告警中心", subtitle: "" },
  analytics: { title: "WAF 报表", subtitle: "" },
  "traffic-dashboard": {
    title: "攻击大盘",
    subtitle: "",
  },
  "rules-crs": { title: "核心规则库", subtitle: "" },
  "rules-ai": { title: "AI 防护", subtitle: "" },
  "rules-custom": {
    title: "签名规则",
    subtitle: "",
  },
  "rules-plugins-json": {
    title: "签名规则",
    subtitle: "",
  },
  "rules-plugins-lua": {
    title: "Lua 插件",
    subtitle: "",
  },
  "ai-semantic": {
    title: "语义设置",
    subtitle: "",
  },
  "ai-triage": {
    title: "智能研判",
    subtitle: "实验室 · LLM 配置",
  },
  "ai-posture": { title: "防护巡检", subtitle: "防护健康度与策略漂移" },
  "ai-policy-learning": {
    title: "策略与学习",
    subtitle: "变更预演与误报样本",
  },
  users: { title: "用户管理", subtitle: "Console 账号与角色" },
  advanced: { title: "高级安全", subtitle: "" },
};

/** Legacy hash `#/modules` compatibility */
export const LEGACY_PAGE_ALIASES: Partial<Record<string, PageId>> = {
  modules: "core-protection",
  "ai-llm": "ai-triage",
  "ai-lab": "ai-triage",
  "ai-policy-sim": "ai-policy-learning",
  "ai-training": "ai-policy-learning",
  "security-graph": "events",
};

/** Platform console shell — CNI / Observer / L3 policy */
export const PLATFORM_CONSOLE_URL = "http://127.0.0.1:5175";
/** Nerve-alert Console (change port when co-hosted with Platform) */
export const ALERT_CONSOLE_URL = "http://127.0.0.1:5175";
export const GATEWAY_DASHBOARD_URL = "http://127.0.0.1:5176";
export const GATEWAY_DASHBOARD_LABEL = "Gateway";

/** Platform network observation (topology) deep link */
export const PLATFORM_OBSERVER_TOPOLOGY_URL = `${PLATFORM_CONSOLE_URL}/#/topology`;
/** Platform full traffic path */
export const PLATFORM_PACKET_JOURNEY_URL = `${PLATFORM_CONSOLE_URL}/#/packet-journey`;
/** Platform CNI deep link */
export const PLATFORM_CNI_URL = `${PLATFORM_CONSOLE_URL}/#/infrastructure`;

/** Rules & plugins — legacy shell injection (React Console merged into protection settings) */
export const RULES_NAV_GROUP = "规则与插件";
export const RULES_NAV: NavItem[] = [];

/** Triage & explain — if NAV is inactive, main.ts injects forcibly (excludes semantic enhancement) */
export const AI_NAV_GROUP = "研判与解释";
export const AI_NAV: NavItem[] = [
  { id: "ai-triage", label: "智能研判", icon: "🧪", group: AI_NAV_GROUP },
  { id: "ai-posture", label: "防护巡检", icon: "🔍", group: AI_NAV_GROUP },
  { id: "ai-policy-learning", label: "策略与学习", icon: "📈", group: AI_NAV_GROUP },
];
