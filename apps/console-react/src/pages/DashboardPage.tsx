import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NerveEmpty,
  NervePageHeader,
  NerveTag,
} from "@nerve/ui";
import { apiGet } from "../api";
import { categoryLabel } from "../lib/labels";
import { PAGE_META, type PageId } from "../nav";
import { siteDisplayName } from "../sites";
import type { ProtectionModules, StatsOverview, WafEvent } from "../lib/waf-types";
import { hourInConsoleZone } from "../lib/datetime";
import { ActionTag, formatTs } from "./shared";

type Props = { onNavigate?: (page: PageId) => void };

type SiteRow = { id: string; name: string; hosts: string[]; enabled: boolean };

const QUICK_LINKS: { page?: PageId; href?: string; icon: string; title: string; desc: string }[] = [
  { page: "events", icon: "▤", title: "攻击日志", desc: "L7 WAF 拦截" },
  { page: "core-protection", icon: "🛡", title: "核心防护", desc: "Web 攻击模块" },
  { page: "rules-crs", icon: "📋", title: "CRS 规则", desc: "规则集更新/回滚" },
  { page: "assets", icon: "⌂", title: "防护站点", desc: "防护域名" },
  { page: "traffic-dashboard", icon: "◐", title: "攻击大盘", desc: "流量三档 · 应用树" },
  { page: "analytics", icon: "📊", title: "WAF 报表", desc: "攻击统计" },
  { page: "cc", icon: "⏱", title: "CC 防护", desc: "频率限制" },
  { page: "access", icon: "⊘", title: "IP 封禁", desc: "访问控制" },
];

const MODULE_LABELS: Record<string, string> = {
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
};

function hourBuckets(events: WafEvent[]): number[] {
  const buckets = new Array(24).fill(0);
  const now = Date.now();
  for (const e of events) {
    const t = new Date(e.ts).getTime();
    if (now - t > 86400000) continue;
    buckets[hourInConsoleZone(e.ts)]++;
  }
  return buckets;
}

function TrendChart({ buckets }: { buckets: number[] }) {
  const max = Math.max(1, ...buckets);
  return (
    <div className="trend-chart">
      {buckets.map((n, i) => {
        const h = Math.round((n / max) * 100);
        return (
          <div key={i} className="trend-bar-wrap" title={`${i}:00 — ${n} 事件`}>
            <div className="trend-bar" style={{ height: `${Math.max(4, h)}%` }} />
            <span className="trend-tick">{i % 6 === 0 ? i : ""}</span>
          </div>
        );
      })}
    </div>
  );
}

export function DashboardPage({ onNavigate }: Props) {
  const meta = PAGE_META.dashboard;
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<StatsOverview | null>(null);
  const [healthOk, setHealthOk] = useState(false);
  const [modules, setModules] = useState<ProtectionModules | null>(null);
  const [blacklistTotal, setBlacklistTotal] = useState(0);
  const [recent, setRecent] = useState<WafEvent[]>([]);
  const [sites, setSites] = useState<SiteRow[]>([]);
  const [ccEnabled, setCcEnabled] = useState(false);
  const [geoEnabled, setGeoEnabled] = useState(false);
  const [nerveHit, setNerveHit] = useState(0);
  const [nerveIfaces, setNerveIfaces] = useState(0);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [overview, nerve, health, mods, blacklist, recentEv, siteRes, cc, geo] =
        await Promise.all([
          apiGet<{ stats: StatsOverview }>("/api/stats/overview"),
          apiGet<{
            status: { counts?: Record<string, number> };
            policy: { ifaces?: string[] };
          }>("/api/nerve/status").catch(() => null),
          apiGet<{ ok: boolean }>("/api/health"),
          apiGet<ProtectionModules>("/api/modules").catch(() => null),
          apiGet<{ total: number }>("/api/policy/blacklist").catch(() => ({ total: 0 })),
          apiGet<{ events: WafEvent[] }>("/api/events?limit=200&layer=l7").catch(() => ({ events: [] })),
          apiGet<{ sites: SiteRow[] }>("/api/sites").catch(() => ({ sites: [] })),
          apiGet<{ config: { enabled: boolean } }>("/api/cc/config").catch(() => ({ config: { enabled: false } })),
          apiGet<{ config: { enabled: boolean } }>("/api/geo/config").catch(() => ({ config: { enabled: false } })),
        ]);
      setStats(overview.stats);
      setHealthOk(health.ok);
      setModules(mods);
      setBlacklistTotal(blacklist.total);
      setRecent(recentEv.events);
      setSites(siteRes.sites);
      setCcEnabled(cc.config.enabled);
      setGeoEnabled(geo.config.enabled);
      setNerveHit(nerve?.status?.counts?.policy_hit ?? 0);
      setNerveIfaces(nerve?.policy?.ifaces?.length ?? 0);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const buckets = useMemo(() => hourBuckets(recent), [recent]);
  const protectedSites = sites.filter((s) => s.enabled).length;
  const enabledModules = modules ? Object.values(modules.modules).filter((m) => m.enabled).length : 0;
  const totalModules = modules ? Object.keys(modules.modules).length : 0;
  const blockRate =
    stats && stats.total_events > 0
      ? ((stats.blocked / stats.total_events) * 100).toFixed(1)
      : "0.0";
  const score = Math.min(
    100,
    Math.round(
      (healthOk ? 25 : 0) +
        (enabledModules / Math.max(1, totalModules)) * 30 +
        (ccEnabled ? 10 : 0) +
        (protectedSites > 0 ? 10 : 0) +
        35,
    ),
  );
  const scoreTone = score >= 80 ? "good" : score >= 50 ? "warn" : "bad";

  return (
    <>
      <NervePageHeader
        title={meta.title}
        subtitle={meta.subtitle}
        actions={
          <NerveButton size="sm" onClick={() => void load()}>
            ↻ 刷新
          </NerveButton>
        }
      />

      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
      {loading && !stats ? <div className="page-loading">加载态势数据…</div> : null}

      {stats ? (
        <>
          <div className={`shield-banner shield-${scoreTone}`}>
            <div className="shield-left">
              <div className="shield-icon">🛡</div>
              <div>
                <div className="shield-title">
                  防护评分 <strong>{score}</strong> / 100
                </div>
                <div className="shield-sub">
                  {score >= 80
                    ? "防护策略运行良好"
                    : score >= 50
                      ? "部分能力未启用，建议完善配置"
                      : "存在风险，请检查 Agent 与防护模块"}
                </div>
              </div>
            </div>
            <div className="shield-stats">
              <div className="stat-pill stat-blue">
                <div className="stat-pill-val">{protectedSites}</div>
                <div className="stat-pill-lbl">防护站点</div>
              </div>
              <div className="stat-pill stat-green">
                <div className="stat-pill-val">
                  {enabledModules}/{totalModules}
                </div>
                <div className="stat-pill-lbl">启用模块</div>
              </div>
              <div className="stat-pill stat-orange">
                <div className="stat-pill-val">{blacklistTotal}</div>
                <div className="stat-pill-lbl">黑名单</div>
              </div>
              <div className="stat-pill stat-red">
                <div className="stat-pill-val">{blockRate}%</div>
                <div className="stat-pill-lbl">拦截率</div>
              </div>
            </div>
          </div>

          <div className="kpi-row">
            <div className="kpi-card">
              <div className="kpi-icon kpi-blue">Σ</div>
              <div className="kpi-body">
                <div className="kpi-label">安全事件</div>
                <div className="kpi-value">{stats.total_events.toLocaleString()}</div>
                <div className="kpi-sub">近 24h</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-red">⊘</div>
              <div className="kpi-body">
                <div className="kpi-label">已拦截</div>
                <div className="kpi-value kpi-danger">{stats.blocked.toLocaleString()}</div>
                <div className="kpi-sub">拦截 {blockRate}%</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-orange">◎</div>
              <div className="kpi-body">
                <div className="kpi-label">观察记录</div>
                <div className="kpi-value kpi-warn">{stats.logged.toLocaleString()}</div>
                <div className="kpi-sub">检测模式</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-purple">◈</div>
              <div className="kpi-body">
                <div className="kpi-label">eBPF 命中</div>
                <div className="kpi-value">{nerveHit.toLocaleString()}</div>
                <div className="kpi-sub">{nerveIfaces} 网卡</div>
              </div>
            </div>
          </div>

          <div className="dash-grid-3">
            <NerveCard className="dash-panel span-2" title="攻击趋势" tip="24 小时事件分布">
              <TrendChart buckets={buckets} />
            </NerveCard>
            <NerveCard className="dash-panel" title="快捷入口">
              <div className="quick-grid">
                {QUICK_LINKS.map((q) =>
                  q.href ? (
                    <a key={q.title} className="quick-tile" href={q.href} target="_blank" rel="noreferrer">
                      <span className="quick-icon">{q.icon}</span>
                      <strong>{q.title}</strong>
                      <span className="muted">{q.desc}</span>
                    </a>
                  ) : (
                    <button
                      key={q.title}
                      type="button"
                      className="quick-tile"
                      onClick={() => q.page && onNavigate?.(q.page)}
                    >
                      <span className="quick-icon">{q.icon}</span>
                      <strong>{q.title}</strong>
                      <span className="muted">{q.desc}</span>
                    </button>
                  ),
                )}
              </div>
            </NerveCard>
          </div>

          <div className="dash-grid-2">
            <NerveCard className="dash-panel" title="攻击类型 TOP" tip="按分类统计">
              <div className="bar-chart">
                {stats.by_category.length > 0 ? (
                  stats.by_category.slice(0, 8).map((c) => {
                    const max = Math.max(1, ...stats.by_category.map((x) => x.count));
                    const pct = Math.round((c.count / max) * 100);
                    return (
                      <div key={c.category} className="bar-row">
                        <span className="bar-label">{categoryLabel(c.category)}</span>
                        <div className="bar-track">
                          <div className="bar-fill bar-fill-danger" style={{ width: `${pct}%` }} />
                        </div>
                        <span className="bar-count">{c.count}</span>
                      </div>
                    );
                  })
                ) : (
                  <NerveEmpty sm description="暂无攻击分类数据" />
                )}
              </div>
            </NerveCard>
            <NerveCard className="dash-panel" title="防护能力矩阵">
              <div className="mod-matrix">
                {modules ? (
                  Object.entries(modules.modules).map(([k, v]) => (
                    <div key={k} className={`mod-chip ${v.enabled ? "on" : "off"}`} title={k}>
                      <span className="mod-dot" />
                      {MODULE_LABELS[k] ?? k}
                    </div>
                  ))
                ) : (
                  <span className="muted">模块配置未加载</span>
                )}
              </div>
              <div className="dash-cap-row">
                <span className={`cap-item ${ccEnabled ? "on" : ""}`}>CC {ccEnabled ? "开" : "关"}</span>
                <span className={`cap-item ${geoEnabled ? "on" : ""}`}>Geo {geoEnabled ? "开" : "关"}</span>
                <span className={`cap-item ${modules?.mode === "block" ? "on" : ""}`}>
                  {modules?.mode === "block" ? "拦截模式" : "检测模式"}
                </span>
              </div>
            </NerveCard>
          </div>

          <div className="dash-grid-2">
            <NerveCard className="dash-panel" title="接入资产" tip={`${protectedSites} 个站点防护中`}>
              <div className="asset-mini-grid">
                {sites.length > 0 ? (
                  sites.slice(0, 4).map((s) => (
                    <div key={s.id} className={`asset-mini-card ${s.enabled ? "active" : "inactive"}`}>
                      <div className="asset-mini-head">
                        <strong>{s.name}</strong>
                        <NerveTag tone={s.enabled ? "green" : "default"}>
                          {s.enabled ? "防护中" : "已停用"}
                        </NerveTag>
                      </div>
                      <div className="muted small">{s.hosts.join(", ")}</div>
                    </div>
                  ))
                ) : (
                  <NerveEmpty sm description="暂无防护站点" />
                )}
              </div>
            </NerveCard>
            <NerveCard className="dash-panel" title="实时威胁" tip="最新 L7 攻击">
              {recent.length > 0 ? (
                <div className="nerve-threat-list threat-list">
                  {recent.slice(0, 8).map((e) => (
                    <div key={e.id} className="nerve-threat-row threat-row">
                      <div className="nerve-threat-line">
                        <span className="nerve-threat-time threat-time">{formatTs(e.ts).slice(11)}</span>
                        <div className="nerve-threat-meta">
                          <span className="nerve-threat-tags">
                            <span className="app-event-tag" title={e.site_id ?? "default"}>
                              {siteDisplayName(sites, e.site_id)}
                            </span>
                            <ActionTag action={e.action} />
                            <span className="layer-badge layer-l7 nerve-threat-cat">
                              {categoryLabel(e.category)}
                            </span>
                          </span>
                          <code className="nerve-threat-ip threat-ip">{e.src_ip ?? e.dst_ip ?? "—"}</code>
                        </div>
                      </div>
                      {(e.uri ?? e.message) ? (
                        <div className="nerve-threat-detail">
                          <code className="nerve-threat-uri threat-msg muted">
                            {(e.uri ?? e.message ?? "").slice(0, 120)}
                          </code>
                        </div>
                      ) : null}
                    </div>
                  ))}
                </div>
              ) : (
                <NerveEmpty sm description="暂无最近事件" />
              )}
            </NerveCard>
          </div>
        </>
      ) : null}
    </>
  );
}
