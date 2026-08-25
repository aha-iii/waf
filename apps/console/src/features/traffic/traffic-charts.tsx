/** traffic feature: Traffic charts. */
import { Fragment } from "react";
import type {
  TrafficSeriesPoint,
  TrafficSummary,
  TrafficTreeResponse,
} from "../../lib/waf-types";
import { categoryLabel } from "../../lib/labels";
import { siteDisplayName, type ProtectedSite } from "../../sites";

export type TrafficRange = "24h" | "7d" | "30d";

function fmt(n: number): string {
  return n.toLocaleString("zh-CN");
}

function pct(part: number, total: number): number {
  if (total <= 0) return 0;
  return (part / total) * 100;
}

function PieChart({
  clean,
  observed,
  blocked,
}: {
  clean: number;
  observed: number;
  blocked: number;
}) {
  const total = Math.max(1, clean + observed + blocked);
  const slices = [
    { label: "干净放行", value: clean, color: "#52c41a" },
    { label: "观测放行", value: observed, color: "#faad14" },
    { label: "拦截", value: blocked, color: "#ff4d4f" },
  ];
  let offset = 0;
  const r = 80;
  const c = 2 * Math.PI * r;
  return (
    <div className="traffic-pie-wrap">
      <svg viewBox="0 0 200 200" width="200" height="200" className="traffic-pie">
        {slices.map((s) => {
          const dash = (s.value / total) * c;
          const el = (
            <circle
              key={s.label}
              cx="100"
              cy="100"
              r={r}
              fill="none"
              stroke={s.color}
              strokeWidth={36}
              strokeDasharray={`${dash} ${c - dash}`}
              strokeDashoffset={-offset}
              transform="rotate(-90 100 100)"
            />
          );
          offset += dash;
          return el;
        })}
        <text x="100" y="96" textAnchor="middle" className="traffic-pie-total">
          {fmt(total)}
        </text>
        <text x="100" y="118" textAnchor="middle" className="traffic-pie-sub">
          总请求
        </text>
      </svg>
      <ul className="traffic-pie-legend">
        {slices.map((s) => (
          <li key={s.label}>
            <i style={{ background: s.color }} />
            {s.label} {fmt(s.value)} ({pct(s.value, total).toFixed(1)}%)
          </li>
        ))}
      </ul>
    </div>
  );
}

function StackSeriesChart({ series }: { series: TrafficSeriesPoint[] }) {
  if (!series.length) {
    return <p className="muted">暂无时序数据</p>;
  }
  const max = Math.max(1, ...series.map((p) => p.total));
  return (
    <div className="traffic-stack-series">
      {series.map((p) => {
        const h = Math.max(4, Math.round((p.total / max) * 100));
        const cleanH = p.total > 0 ? (p.clean / p.total) * h : 0;
        const obsH = p.total > 0 ? (p.observed / p.total) * h : 0;
        const blkH = p.total > 0 ? (p.blocked / p.total) * h : 0;
        return (
          <div key={p.bucket} className="traffic-stack-col" title={`${p.bucket}: ${fmt(p.total)}`}>
            <div className="traffic-stack-bar" style={{ height: `${h}%` }}>
              <div style={{ height: `${cleanH}%`, background: "#52c41a" }} />
              <div style={{ height: `${obsH}%`, background: "#faad14" }} />
              <div style={{ height: `${blkH}%`, background: "#ff4d4f" }} />
            </div>
            <span className="traffic-stack-tick">{p.bucket.slice(-5)}</span>
          </div>
        );
      })}
    </div>
  );
}

type TreeProps = {
  tree: TrafficTreeResponse;
  sites: ProtectedSite[];
  expandedSites: Set<string>;
  expandedHosts: Set<string>;
  onToggleSite: (id: string) => void;
  onToggleHost: (key: string) => void;
};

function TrafficTreeTable({
  tree,
  sites,
  expandedSites,
  expandedHosts,
  onToggleSite,
  onToggleHost,
}: TreeProps) {
  if (!tree.sites.length) {
    return <p className="muted">暂无流量数据（Edge 计数约 1 分钟后可见）</p>;
  }
  return (
    <table className="ant-table traffic-tree-table">
      <thead>
        <tr>
          <th>应用 / 域名 / 攻击类型</th>
          <th>总请求</th>
          <th>干净</th>
          <th>观测</th>
          <th>拦截</th>
          <th>拦截率</th>
        </tr>
      </thead>
      <tbody>
        {tree.sites.map((site) => {
          const siteOpen = expandedSites.has(site.site_id);
          const siteName = siteDisplayName(sites, site.site_id);
          const siteRate =
            site.total > 0 ? ((site.blocked / site.total) * 100).toFixed(2) : "0.00";
          return (
            <Fragment key={`site-${site.site_id}`}>
              <tr
                className="traffic-tree-site"
                onClick={() => onToggleSite(site.site_id)}
              >
                <td>
                  <span className="traffic-tree-toggle">{siteOpen ? "▼" : "▶"}</span>
                  {siteName}
                  <span className="muted small"> ({site.site_id})</span>
                </td>
                <td>{fmt(site.total)}</td>
                <td>{fmt(site.clean)}</td>
                <td>{fmt(site.observed)}</td>
                <td>{fmt(site.blocked)}</td>
                <td>{siteRate}%</td>
              </tr>
              {siteOpen &&
                site.hosts.map((host) => {
                  const hkey = `${site.site_id}|${host.host}`;
                  const hostOpen = expandedHosts.has(hkey);
                  const hRate =
                    host.total > 0
                      ? ((host.blocked / host.total) * 100).toFixed(2)
                      : "0.00";
                  return (
                    <Fragment key={`host-${hkey}`}>
                      <tr
                        className="traffic-tree-host"
                        onClick={(e) => {
                          e.stopPropagation();
                          onToggleHost(hkey);
                        }}
                      >
                        <td>
                          <span className="traffic-tree-indent" />
                          <span className="traffic-tree-toggle">{hostOpen ? "▼" : "▶"}</span>
                          {host.host || "(unknown host)"}
                        </td>
                        <td>{fmt(host.total)}</td>
                        <td>{fmt(host.clean)}</td>
                        <td>{fmt(host.observed)}</td>
                        <td>{fmt(host.blocked)}</td>
                        <td>{hRate}%</td>
                      </tr>
                      {hostOpen &&
                        host.attack_categories.map((cat) => (
                          <tr key={`cat-${hkey}-${cat.category}`} className="traffic-tree-cat">
                            <td>
                              <span className="traffic-tree-indent2" />
                              {categoryLabel(cat.category)}
                              <span className="muted small"> ({cat.category})</span>
                            </td>
                            <td colSpan={4} />
                            <td>{fmt(cat.count)}</td>
                          </tr>
                        ))}
                    </Fragment>
                  );
                })}
            </Fragment>
          );
        })}
      </tbody>
    </table>
  );
}

export { PieChart, StackSeriesChart, TrafficTreeTable, fmt, pct };
export type { TrafficSummary };
