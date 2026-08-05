import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveCard,
  NervePageHeader,
  NervePagination,
  NerveTable,
  defaultPaginationState,
  paginate,
} from "@nerve/ui";
import { apiGet } from "../api";
import { categoryLabel } from "../lib/labels";
import { useToast } from "../hooks/useToast";
import type { StatsOverview } from "../lib/waf-types";

type FpAnalytics = {
  days: number;
  total_blocks: number;
  confirmed_fp: number;
  false_positive_rate: number;
  top_rules: { rule_id: string; count: number }[];
  by_site: { site_id: string; count: number }[];
};

export function AnalyticsPage(): JSX.Element {
  const toast = useToast();
  const [stats, setStats] = useState<StatsOverview | null>(null);
  const [fp, setFp] = useState<FpAnalytics | null>(null);
  const [error, setError] = useState("");
  const [catPage, setCatPage] = useState(defaultPaginationState());
  const [ipPage, setIpPage] = useState(defaultPaginationState());
  const [rulePage, setRulePage] = useState(defaultPaginationState());
  const [sitePage, setSitePage] = useState(defaultPaginationState());

  const load = useCallback(async () => {
    const [{ stats: overview }, fpData] = await Promise.all([
      apiGet<{ stats: StatsOverview }>("/api/stats/overview"),
      apiGet<FpAnalytics>("/api/analytics/false-positives?days=7"),
    ]);
    setStats(overview);
    setFp(fpData);
  }, []);

  useEffect(() => {
    void load().catch((e) => {
      setError(String(e));
      toast(String(e), false);
    });
  }, [load, toast]);

  if (error) {
    return (
      <>
        <NervePageHeader title="WAF 报表" />
        <NerveAlert tone="error">{error}</NerveAlert>
      </>
    );
  }

  if (!stats || !fp) return <p className="muted">加载中…</p>;

  return (
    <>
      <NervePageHeader title="WAF 报表" />
      <div className="kpi-row">
        <div className="kpi-card">
          <div className="kpi-icon kpi-blue">Σ</div>
          <div className="kpi-body">
            <div className="kpi-label">事件</div>
            <div className="kpi-value">{stats.total_events}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-red">⊘</div>
          <div className="kpi-body">
            <div className="kpi-label">拦截</div>
            <div className="kpi-value kpi-danger">{stats.blocked}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-orange">◎</div>
          <div className="kpi-body">
            <div className="kpi-label">记录</div>
            <div className="kpi-value">{stats.logged}</div>
          </div>
        </div>
        <div className="kpi-card">
          <div className="kpi-icon kpi-green">✓</div>
          <div className="kpi-body">
            <div className="kpi-label">误报确认（7天）</div>
            <div className="kpi-value">{fp.confirmed_fp}</div>
            <div className="kpi-sub muted small">误报率 {fp.false_positive_rate}%</div>
          </div>
        </div>
      </div>

      <div className="dash-grid-2">
        <NerveCard title="攻击类型">
          <NerveTable
            columns={[
              { key: "category", title: "类型", render: (r) => categoryLabel(r.category) },
              { key: "count", title: "次数" },
            ]}
            dataSource={paginate(stats.by_category, catPage.page, catPage.pageSize)}
            rowKey="category"
            emptyText="暂无分类统计"
          />
          <NervePagination total={stats.by_category.length} page={catPage.page} pageSize={catPage.pageSize} onChange={setCatPage} />
        </NerveCard>

        <NerveCard title="攻击源 TOP">
          <NerveTable
            columns={[
              { key: "src_ip", title: "IP", render: (r) => <code>{r.src_ip}</code> },
              { key: "count", title: "次数" },
            ]}
            dataSource={paginate(stats.top_ips, ipPage.page, ipPage.pageSize)}
            rowKey="src_ip"
            emptyText="暂无 IP 统计"
          />
          <NervePagination total={stats.top_ips.length} page={ipPage.page} pageSize={ipPage.pageSize} onChange={setIpPage} />
        </NerveCard>
      </div>

      <div className="dash-grid-2" style={{ marginTop: 16 }}>
        <NerveCard title="误报 Top 规则（7天）">
          <NerveTable
            columns={[
              { key: "rule_id", title: "规则 ID", render: (r) => <code>{r.rule_id}</code> },
              { key: "count", title: "确认次数" },
            ]}
            dataSource={paginate(fp.top_rules, rulePage.page, rulePage.pageSize)}
            rowKey="rule_id"
            emptyText="暂无误报确认记录"
          />
          <NervePagination total={fp.top_rules.length} page={rulePage.page} pageSize={rulePage.pageSize} onChange={setRulePage} />
        </NerveCard>

        <NerveCard title="误报按应用（7天）">
          <NerveTable
            columns={[
              { key: "site_id", title: "应用", render: (r) => <code>{r.site_id}</code> },
              { key: "count", title: "确认次数" },
            ]}
            dataSource={paginate(fp.by_site, sitePage.page, sitePage.pageSize)}
            rowKey="site_id"
            emptyText="暂无按应用统计"
          />
          <NervePagination total={fp.by_site.length} page={sitePage.page} pageSize={sitePage.pageSize} onChange={setSitePage} />
        </NerveCard>
      </div>
    </>
  );
}
