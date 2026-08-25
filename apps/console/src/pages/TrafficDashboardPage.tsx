/** Console page: live attack traffic dashboard. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NervePageHeader,
  NerveSelect,
  NerveTableToolbar,
} from "@nerve/ui";
import { apiGet } from "../api";
import { getAuthHeaders } from "../auth";
import {
  PieChart,
  StackSeriesChart,
  TrafficTreeTable,
  type TrafficRange,
} from "../features/traffic/traffic-charts";
import { useToast } from "../hooks/useToast";
import type {
  TrafficSeriesPoint,
  TrafficSummary,
  TrafficTreeResponse,
} from "../lib/waf-types";
import { loadSites, type ProtectedSite } from "../sites";

function fmt(n: number): string {
  return n.toLocaleString("zh-CN");
}

export function TrafficDashboardPage(): JSX.Element {
  const toast = useToast();
  const [range, setRange] = useState<TrafficRange>("24h");
  const [siteId, setSiteId] = useState("");
  const [sites, setSites] = useState<ProtectedSite[]>([]);
  const [summary, setSummary] = useState<TrafficSummary | null>(null);
  const [series, setSeries] = useState<TrafficSeriesPoint[]>([]);
  const [tree, setTree] = useState<TrafficTreeResponse | null>(null);
  const [error, setError] = useState("");
  const [expandedSites, setExpandedSites] = useState<Set<string>>(new Set());
  const [expandedHosts, setExpandedHosts] = useState<Set<string>>(new Set());

  const q = useMemo(() => {
    const p = new URLSearchParams({ since: range });
    if (siteId.trim()) p.set("site_id", siteId.trim());
    if (range === "7d" || range === "30d") p.set("granularity", "day");
    return p.toString();
  }, [range, siteId]);

  const load = useCallback(async () => {
    const [siteList, sumRes, seriesRes, treeRes] = await Promise.all([
      loadSites(),
      apiGet<{ summary: TrafficSummary }>(`/api/stats/traffic?${q}`),
      apiGet<{ series: TrafficSeriesPoint[] }>(`/api/stats/traffic/series?${q}`),
      apiGet<TrafficTreeResponse>(`/api/stats/traffic/tree?${q}`),
    ]);
    setSites(siteList);
    setSummary(sumRes.summary);
    setSeries(seriesRes.series);
    setTree(treeRes);
    setError("");
  }, [q]);

  useEffect(() => {
    void load().catch((e) => {
      setError(String(e));
      toast(String(e), false);
    });
  }, [load, toast]);

  const exportCsv = async () => {
    try {
      const res = await fetch(`/api/stats/traffic/export.csv?${q}`, {
        headers: getAuthHeaders(),
      });
      if (!res.ok) throw new Error(`导出失败 HTTP ${res.status}`);
      const blob = await res.blob();
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `waf-traffic-${range}.csv`;
      a.click();
      URL.revokeObjectURL(a.href);
      toast("CSV 已下载", true);
    } catch (e) {
      toast(String(e), false);
    }
  };

  const toggleSite = (id: string) => {
    setExpandedSites((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleHost = (key: string) => {
    setExpandedHosts((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  if (error) {
    return (
      <>
        <NervePageHeader title="攻击大盘" />
        <NerveAlert tone="error">{error}</NerveAlert>
      </>
    );
  }

  if (!summary || !tree) {
    return (
      <>
        <NervePageHeader title="攻击大盘" />
        <p className="muted">加载中…</p>
      </>
    );
  }

  const siteOptions = [
    { value: "", label: "全部应用" },
    ...sites.map((s) => ({ value: s.id, label: `${s.name} (${s.id})` })),
  ];

  return (
    <>
      <NervePageHeader title="攻击大盘" />
      <NerveTableToolbar>
        <NerveSelect
          value={range}
          options={[
            { value: "24h", label: "近 24 小时" },
            { value: "7d", label: "近 7 天" },
            { value: "30d", label: "近 30 天" },
          ]}
          onChange={(e) => setRange(e.target.value as TrafficRange)}
        />
        <NerveSelect
          value={siteId}
          options={siteOptions}
          onChange={(e) => setSiteId(e.target.value)}
        />
        <NerveButton onClick={() => void load()}>刷新</NerveButton>
        <NerveButton variant="primary" onClick={() => void exportCsv()}>
          导出 CSV
        </NerveButton>
      </NerveTableToolbar>

      <div className="kpi-row">
        <div className="kpi-card">
          <div className="kpi-icon kpi-blue">Σ</div>
          <div className="kpi-body">
            <div className="kpi-label">总请求</div>
            <div className="kpi-value">{fmt(summary.total)}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-green">✓</div>
          <div className="kpi-body">
            <div className="kpi-label">干净放行</div>
            <div className="kpi-value">{fmt(summary.clean)}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-orange">◎</div>
          <div className="kpi-body">
            <div className="kpi-label">观测放行</div>
            <div className="kpi-value">{fmt(summary.observed)}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-red">⊘</div>
          <div className="kpi-body">
            <div className="kpi-label">拦截</div>
            <div className="kpi-value kpi-danger">{fmt(summary.blocked)}</div>
            <div className="kpi-sub muted small">拦截率 {summary.block_rate.toFixed(2)}%</div>
          </div>
        </div>
      </div>

      <div className="dash-grid-2">
        <NerveCard title="流量构成（饼图）">
          <PieChart clean={summary.clean} observed={summary.observed} blocked={summary.blocked} />
        </NerveCard>
        <NerveCard title="时序堆叠（干净 / 观测 / 拦截）">
          <StackSeriesChart series={series} />
        </NerveCard>
      </div>

      <NerveCard title="应用 → 域名 → 攻击类型">
        <p className="muted small" style={{ marginBottom: 12 }}>
          点击行展开；第三级攻击类型来自拦截事件（block/challenge）。
        </p>
        <TrafficTreeTable
          tree={tree}
          sites={sites}
          expandedSites={expandedSites}
          expandedHosts={expandedHosts}
          onToggleSite={toggleSite}
          onToggleHost={toggleHost}
        />
      </NerveCard>
    </>
  );
}
