import { apiGet } from "./api.js";
import { actionTag, categoryLabel, esc, nerveThreatList, nerveThreatRow, pageHeader, statPill } from "./components/ui.js";
import { PAGE_META, PLATFORM_CONSOLE_URL, PLATFORM_OBSERVER_TOPOLOGY_URL } from "./nav.js";
import { siteDisplayName } from "./sites.js";
import type { ProtectionModules, StatsOverview, WafEvent } from "./security-view.js";

type QuickLink = { page?: string; href?: string; icon: string; title: string; desc: string };

const QUICK_LINKS: QuickLink[] = [
  { page: "events", icon: "▤", title: "攻击日志", desc: "L7 WAF 拦截" },
  { href: PLATFORM_OBSERVER_TOPOLOGY_URL, icon: "◈", title: "网络观测 →", desc: "Platform · Observer" },
  { page: "modules", icon: "◆", title: "核心防护", desc: "Web 攻击模块" },
  { page: "rules-crs", icon: "📋", title: "CRS 规则", desc: "规则集更新/回滚" },
  { page: "assets", icon: "⌂", title: "防护站点", desc: "防护域名" },
  { page: "analytics", icon: "📊", title: "WAF 报表", desc: "攻击统计" },
  { page: "cc", icon: "⏱", title: "CC 防护", desc: "频率限制" },
  { page: "access", icon: "⊘", title: "IP 封禁", desc: "访问控制" },
];

function hourBuckets(events: WafEvent[]): number[] {
  const buckets = new Array(24).fill(0);
  const now = Date.now();
  for (const e of events) {
    const t = new Date(e.ts).getTime();
    if (now - t > 86400000) continue;
    const h = new Date(e.ts).getHours();
    buckets[h]++;
  }
  return buckets;
}

function renderTrendChart(buckets: number[]): string {
  const max = Math.max(1, ...buckets);
  const bars = buckets
    .map((n, i) => {
      const h = Math.round((n / max) * 100);
      const blocked = i >= 0;
      return `<div class="trend-bar-wrap" title="${i}:00 — ${n} 事件">
        <div class="trend-bar ${blocked ? "" : "muted"}" style="height:${Math.max(4, h)}%"></div>
        <span class="trend-tick">${i % 6 === 0 ? i : ""}</span>
      </div>`;
    })
    .join("");
  return `<div class="trend-chart">${bars}</div>`;
}

function moduleMatrix(modules: ProtectionModules | null): string {
  if (!modules) return `<div class="muted">模块配置未加载</div>`;
  const labels: Record<string, string> = {
    sqli: "SQLi", xss: "XSS", rce: "RCE", path_traversal: "路径", ssrf: "SSRF",
    file_upload: "上传", webshell: "Shell", info_leak: "泄露", bot: "Bot", cc: "CC",
  };
  return Object.entries(modules.modules)
    .map(([k, v]) => {
      const on = v.enabled;
      return `<div class="mod-chip ${on ? "on" : "off"}" title="${k}">
        <span class="mod-dot"></span>${esc(labels[k] ?? k)}
      </div>`;
    })
    .join("");
}

export async function renderCloudDashboard(
  root: HTMLElement,
  navigate: (page: string) => void,
): Promise<void> {
  const meta = PAGE_META.dashboard;
  root.innerHTML = `
    ${pageHeader(meta.title, meta.subtitle, `<button type="button" class="ant-btn ant-btn-sm" id="dash-refresh">↻ 刷新</button><a class="ant-btn ant-btn-sm" href="${PLATFORM_CONSOLE_URL}" target="_blank" rel="noopener">→ 平台 Console</a>`)}
    <div id="dash-body"><div class="page-loading">加载态势数据…</div></div>`;

  const body = root.querySelector("#dash-body")!;

  const load = async () => {
    body.innerHTML = `<div class="page-loading">加载态势数据…</div>`;
    const [overview, nerve, health, modules, blacklist, recent, networkRecent, sites, cc, geo] =
      await Promise.all([
        apiGet<{ stats: StatsOverview; backend: string }>("/api/stats/overview"),
        apiGet<{ status: { counts?: Record<string, number> }; policy: { ifaces?: string[] }; blacklist: { entries: unknown[] } }>(
          "/api/nerve/status",
        ).catch(() => null),
        apiGet<{ ok: boolean; nerveUrl: string; configBackend: string; eventBackend: string; databaseConfigured: boolean }>(
          "/api/health",
        ),
        apiGet<ProtectionModules>("/api/modules").catch(() => null),
        apiGet<{ total: number }>("/api/policy/blacklist").catch(() => ({ total: 0 })),
        apiGet<{ events: WafEvent[] }>("/api/events?limit=200&layer=l7").catch(() => ({ events: [] })),
        apiGet<{ events: WafEvent[] }>("/api/events?limit=200&layer=l3").catch(() => ({ events: [] })),
        apiGet<{ sites: { id: string; name: string; hosts: string[]; enabled: boolean }[] }>("/api/sites").catch(() => ({ sites: [] })),
        apiGet<{ config: { enabled: boolean } }>("/api/cc/config").catch(() => ({ config: { enabled: false } })),
        apiGet<{ config: { enabled: boolean; block_countries: string[] } }>("/api/geo/config").catch(() => ({
          config: { enabled: false, block_countries: [] },
        })),
      ]);

    const { stats, backend } = overview;
    const blockRate = stats.total_events > 0 ? ((stats.blocked / stats.total_events) * 100).toFixed(1) : "0.0";
    const enabledModules = modules ? Object.values(modules.modules).filter((m) => m.enabled).length : 0;
    const totalModules = modules ? Object.keys(modules.modules).length : 0;
    const protectedSites = sites.sites.filter((s) => s.enabled).length;
    const score = Math.min(
      100,
      Math.round(
        (health.ok ? 25 : 0) +
          (nerve ? 25 : 0) +
          (enabledModules / Math.max(1, totalModules)) * 30 +
          (cc.config.enabled ? 10 : 0) +
          (protectedSites > 0 ? 10 : 0),
      ),
    );
    const scoreTone = score >= 80 ? "good" : score >= 50 ? "warn" : "bad";
    const buckets = hourBuckets(recent.events);
    const netOrphan = networkRecent.events.filter((e) => e.category === "orphan_connect").length;
    const netHigh = networkRecent.events.filter((e) => e.severity === "high" || e.severity === "critical").length;
    const netScan = networkRecent.events.filter((e) => e.category === "port_scan").length;

    const shieldBanner = `
      <div class="shield-banner shield-${scoreTone}">
        <div class="shield-left">
          <div class="shield-icon">🛡</div>
          <div>
            <div class="shield-title">防护评分 <strong>${score}</strong> / 100</div>
            <div class="shield-sub">${score >= 80 ? "防护策略运行良好" : score >= 50 ? "部分能力未启用，建议完善配置" : "存在风险，请检查 Agent 与防护模块"}</div>
          </div>
        </div>
        <div class="shield-stats">
          ${statPill("防护站点", protectedSites, "blue")}
          ${statPill("启用模块", `${enabledModules}/${totalModules}`, "green")}
          ${statPill("黑名单", blacklist.total, "orange")}
          ${statPill("拦截率", `${blockRate}%`, "red")}
        </div>
      </div>`;

    const kpiCards = `
      <div class="kpi-card"><div class="kpi-icon kpi-blue">Σ</div><div class="kpi-body">
        <div class="kpi-label">安全事件</div><div class="kpi-value">${stats.total_events.toLocaleString()}</div>
        <div class="kpi-sub">近 24h · ${esc(backend)}</div></div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-red">⊘</div><div class="kpi-body">
        <div class="kpi-label">已拦截</div><div class="kpi-value kpi-danger">${stats.blocked.toLocaleString()}</div>
        <div class="kpi-sub">拦截 ${blockRate}%</div></div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-orange">◎</div><div class="kpi-body">
        <div class="kpi-label">观察记录</div><div class="kpi-value kpi-warn">${stats.logged.toLocaleString()}</div>
        <div class="kpi-sub">检测模式</div></div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-purple">◈</div><div class="kpi-body">
        <div class="kpi-label">eBPF 命中</div><div class="kpi-value">${(nerve?.status?.counts?.policy_hit ?? 0).toLocaleString()}</div>
        <div class="kpi-sub">${nerve?.policy?.ifaces?.length ?? 0} 网卡</div></div></div>`;

    const quickNav = QUICK_LINKS.map((q) => {
      if (q.href) {
        return `<a class="quick-tile" href="${esc(q.href)}" target="_blank" rel="noopener">
        <span class="quick-icon">${q.icon}</span>
        <strong>${esc(q.title)}</strong><span class="muted">${esc(q.desc)}</span>
      </a>`;
      }
      return `<button type="button" class="quick-tile" data-goto="${esc(q.page!)}">
        <span class="quick-icon">${q.icon}</span>
        <strong>${esc(q.title)}</strong><span class="muted">${esc(q.desc)}</span>
      </button>`;
    }).join("");

    const maxCat = Math.max(1, ...stats.by_category.map((c) => c.count));
    const catBars =
      stats.by_category.length > 0
        ? stats.by_category.slice(0, 8).map((c) => {
            const pct = Math.round((c.count / maxCat) * 100);
            return `<div class="bar-row"><span class="bar-label">${esc(categoryLabel(c.category))}</span>
              <div class="bar-track"><div class="bar-fill bar-fill-danger" style="width:${pct}%"></div></div>
              <span class="bar-count">${c.count}</span></div>`;
          }).join("")
        : `<div class="ant-empty-sm muted">暂无攻击分类数据</div>`;

    const siteCards =
      sites.sites.length > 0
        ? sites.sites.slice(0, 4).map((s) => `<div class="asset-mini-card ${s.enabled ? "active" : "inactive"}">
            <div class="asset-mini-head"><strong>${esc(s.name)}</strong>
              <span class="ant-tag ${s.enabled ? "ant-tag-success" : ""}">${s.enabled ? "防护中" : "已停用"}</span></div>
            <div class="muted small">${esc(s.hosts.join(", "))}</div>
          </div>`).join("")
        : `<div class="ant-empty-sm muted">暂无防护站点</div>`;

    const recentRows =
      recent.events.length > 0
        ? nerveThreatList(
            recent.events
              .slice(0, 8)
              .map((e) =>
                nerveThreatRow({
                  time: e.ts.slice(11, 19),
                  siteLabel: siteDisplayName(sites.sites, e.site_id),
                  siteId: e.site_id,
                  actionHtml: actionTag(e.action),
                  categoryLabel: categoryLabel(e.category),
                  ip: e.src_ip ?? e.dst_ip ?? "—",
                  uri: (e.uri ?? e.message ?? "").slice(0, 120),
                }),
              )
              .join(""),
          )
        : `<div class="ant-empty-sm muted">暂无最近事件</div>`;

    body.innerHTML = `
      ${shieldBanner}
      <div class="kpi-row">${kpiCards}</div>
      <div class="dash-grid-3">
        <div class="ant-card dash-panel span-2">
          <div class="ant-card-head">攻击趋势<span class="head-tip">24 小时事件分布</span></div>
          <div class="ant-card-body">${renderTrendChart(buckets)}</div>
        </div>
        <div class="ant-card dash-panel">
          <div class="ant-card-head">快捷入口</div>
          <div class="ant-card-body quick-grid">${quickNav}</div>
        </div>
      </div>
      <div class="dash-grid-2">
        <div class="ant-card dash-panel">
          <div class="ant-card-head">攻击类型 TOP<span class="head-tip">按分类统计</span></div>
          <div class="ant-card-body bar-chart">${catBars}</div>
        </div>
        <div class="ant-card dash-panel">
          <div class="ant-card-head">防护能力矩阵</div>
          <div class="ant-card-body">
            <div class="mod-matrix">${moduleMatrix(modules)}</div>
            <div class="dash-cap-row">
              <span class="cap-item ${cc.config.enabled ? "on" : ""}">CC ${cc.config.enabled ? "开" : "关"}</span>
              <span class="cap-item ${geo.config.enabled ? "on" : ""}">Geo ${geo.config.enabled ? "开" : "关"}</span>
              <span class="cap-item ${modules?.mode === "block" ? "on" : ""}">${modules?.mode === "block" ? "拦截模式" : "检测模式"}</span>
            </div>
          </div>
        </div>
      </div>
      <div class="dash-grid-2">
        <div class="ant-card dash-panel">
          <div class="ant-card-head">接入资产<span class="head-tip">${protectedSites} 个站点防护中</span></div>
          <div class="ant-card-body asset-mini-grid">${siteCards}</div>
        </div>
        <div class="ant-card dash-panel">
          <div class="ant-card-head">实时威胁<span class="head-tip">最新 L7 攻击</span></div>
          <div class="ant-card-body">${recentRows}</div>
        </div>
      </div>
      <div class="dash-grid-2">
        <div class="ant-card dash-panel">
          <div class="ant-card-head">网络观测<span class="head-tip">Agent L3 · 样本 ${networkRecent.events.length} 条</span></div>
          <div class="ant-card-body">
            <div class="kpi-row" style="margin:0">
              <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">孤儿连接</div><div class="kpi-value kpi-warn">${netOrphan}</div></div></div>
              <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">高危/组合</div><div class="kpi-value kpi-danger">${netHigh}</div></div></div>
              <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">端口扫描</div><div class="kpi-value">${netScan}</div></div></div>
            </div>
            <p class="muted small" style="margin-top:12px"><a class="ant-btn-link" href="${PLATFORM_OBSERVER_TOPOLOGY_URL}" target="_blank" rel="noopener">打开 Platform 网络观测 →</a></p>
          </div>
        </div>
      </div>`;

    body.querySelectorAll<HTMLElement>("[data-goto]").forEach((btn) => {
      btn.addEventListener("click", () => navigate(btn.dataset.goto!));
    });
  };

  root.querySelector("#dash-refresh")?.addEventListener("click", () => {
    void load().catch((e) => {
      body.innerHTML = `<div class="ant-alert ant-alert-error">${e instanceof Error ? e.message : String(e)}</div>`;
    });
  });

  await load();
}
