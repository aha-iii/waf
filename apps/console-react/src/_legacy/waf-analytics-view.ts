import { apiGet } from "./api.js";

import { categoryLabel, esc, mountPaginatedTable, pageHeader } from "./components/ui.js";

import type { StatsOverview } from "./security-view.js";



type FpAnalytics = {

  days: number;

  total_blocks: number;

  confirmed_fp: number;

  false_positive_rate: number;

  top_rules: { rule_id: string; count: number }[];

  by_site: { site_id: string; count: number }[];

};



export async function renderWafAnalyticsView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {

  root.innerHTML = `${pageHeader("WAF 报表", "")}<div id="waf-analytics">加载中…</div>`;

  const body = root.querySelector("#waf-analytics")!;



  try {

    const [{ stats }, fp] = await Promise.all([

      apiGet<{ stats: StatsOverview }>("/api/stats/overview"),

      apiGet<FpAnalytics>("/api/analytics/false-positives?days=7"),

    ]);

    body.innerHTML = `

      <div class="kpi-row">

        <div class="kpi-card"><div class="kpi-icon kpi-blue">Σ</div><div class="kpi-body"><div class="kpi-label">事件</div><div class="kpi-value">${stats.total_events}</div></div></div>

        <div class="kpi-card"><div class="kpi-icon kpi-red">⊘</div><div class="kpi-body"><div class="kpi-label">拦截</div><div class="kpi-value kpi-danger">${stats.blocked}</div></div></div>

        <div class="kpi-card"><div class="kpi-icon kpi-orange">◎</div><div class="kpi-body"><div class="kpi-label">记录</div><div class="kpi-value">${stats.logged}</div></div></div>

        <div class="kpi-card"><div class="kpi-icon kpi-green">✓</div><div class="kpi-body"><div class="kpi-label">误报确认（7天）</div><div class="kpi-value">${fp.confirmed_fp}</div><div class="kpi-sub muted small">误报率 ${fp.false_positive_rate}%</div></div></div>

      </div>

      <div class="dash-grid-2">

        <div class="ant-card"><div class="ant-card-head">攻击类型</div><div class="ant-card-body" id="wa-cat-table"></div></div>

        <div class="ant-card"><div class="ant-card-head">攻击源 TOP</div><div class="ant-card-body" id="wa-ip-table"></div></div>

      </div>

      <div class="dash-grid-2" style="margin-top:16px">

        <div class="ant-card"><div class="ant-card-head">误报 Top 规则（7天）</div><div class="ant-card-body" id="wa-fp-rules"></div></div>

        <div class="ant-card"><div class="ant-card-head">误报按应用（7天）</div><div class="ant-card-body" id="wa-fp-sites"></div></div>

      </div>`;



    mountPaginatedTable(body.querySelector("#wa-cat-table")!, "wa-cat", stats.by_category, {

      tableClass: "ant-table",

      headHtml: "<thead><tr><th>类型</th><th>次数</th></tr></thead>",

      rowHtml: (c) => `<tr><td>${esc(categoryLabel(c.category))}</td><td>${c.count}</td></tr>`,

      emptyText: "暂无分类统计",

    });



    mountPaginatedTable(body.querySelector("#wa-ip-table")!, "wa-ip", stats.top_ips, {

      tableClass: "ant-table",

      headHtml: "<thead><tr><th>IP</th><th>次数</th></tr></thead>",

      rowHtml: (ip) => `<tr><td><code>${esc(ip.src_ip)}</code></td><td>${ip.count}</td></tr>`,

      emptyText: "暂无 IP 统计",

    });



    mountPaginatedTable(body.querySelector("#wa-fp-rules")!, "wa-fp-rules", fp.top_rules, {

      tableClass: "ant-table",

      headHtml: "<thead><tr><th>规则 ID</th><th>确认次数</th></tr></thead>",

      rowHtml: (r) => `<tr><td><code>${esc(r.rule_id)}</code></td><td>${r.count}</td></tr>`,

      emptyText: "暂无误报确认记录",

    });



    mountPaginatedTable(body.querySelector("#wa-fp-sites")!, "wa-fp-sites", fp.by_site, {

      tableClass: "ant-table",

      headHtml: "<thead><tr><th>应用</th><th>确认次数</th></tr></thead>",

      rowHtml: (s) => `<tr><td><code>${esc(s.site_id)}</code></td><td>${s.count}</td></tr>`,

      emptyText: "暂无按应用统计",

    });

  } catch (e) {

    body.innerHTML = `<div class="ant-alert ant-alert-error">${e instanceof Error ? e.message : String(e)}</div>`;

    toast(String(e), false);

  }

}

