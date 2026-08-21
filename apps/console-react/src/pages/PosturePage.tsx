/** Console page: protection posture health and drift checks. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NervePageHeader,
  NerveSelect,
  NerveTable,
  NerveTag,
} from "@nerve/ui";
import { apiGet, apiPost } from "../api";
import {
  PostureCheckDetailModal,
  type PostureCheckRow,
} from "../features/posture/PostureCheckDetailModal";
import { formatTs } from "../lib/datetime";
import { useToast } from "../hooks/useToast";
import { loadSites, siteDisplayName, type ProtectedSite } from "../sites";

type PostureCheck = PostureCheckRow & {
  category: string;
  code: string;
};

type PostureReport = {
  ok: boolean;
  scanned_at?: string;
  window_days?: number;
  overall_score: number;
  site_scores: Record<string, number>;
  checks?: PostureCheck[];
  scope?: { site_ids?: string[]; dimensions?: string[] };
  findings: {
    severity: string;
    code: string;
    message: string;
    site_id?: string;
    recommendation: string;
  }[];
  summary: {
    sites_scanned: number;
    findings_count: number;
    checks_count?: number;
    checks_pass?: number;
    modules_off: number;
    total_disabled_rules: number;
  };
  engram?: { detect_model?: boolean; llm_explain?: boolean };
  ai_policy: {
    hot_path_ai_block: boolean;
    model_assist_only: boolean;
    changes_require_confirm: boolean;
    shadow_before_prod: boolean;
  };
  scan_id?: string;
};

export function PosturePage() {
  const toast = useToast();
  const [sites, setSites] = useState<ProtectedSite[]>([]);
  const [siteFilter, setSiteFilter] = useState("");
  const [report, setReport] = useState<PostureReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [detailCheck, setDetailCheck] = useState<PostureCheck | null>(null);

  useEffect(() => {
    void loadSites()
      .then(setSites)
      .catch(() => setSites([]));
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const q = siteFilter ? `?site_id=${encodeURIComponent(siteFilter)}` : "";
      const data = await apiGet<PostureReport>(`/api/posture/scan${q}`);
      setReport(data);
    } catch (e) {
      const msg = String(e);
      setLoadError(msg);
      setReport(null);
      toast(msg, false);
    } finally {
      setLoading(false);
    }
  }, [siteFilter, toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const runScan = async () => {
    setScanning(true);
    try {
      const q = siteFilter ? `?site_id=${encodeURIComponent(siteFilter)}&persist=true` : "?persist=true";
      const data = await apiPost<PostureReport>(`/api/posture/scan${q}`, {});
      setReport(data);
      toast("巡检完成", true);
    } catch (e) {
      toast(String(e), false);
    } finally {
      setScanning(false);
    }
  };

  const severityTone = (s: string) =>
    s === "high" ? "red" : s === "medium" ? "orange" : "default";

  const checkTone = (s: string) => {
    if (s === "pass") return "green";
    if (s === "fail") return "red";
    if (s === "warn") return "orange";
    return "default";
  };

  const checkLabel = (s: string) => {
    if (s === "pass") return "通过";
    if (s === "fail") return "未通过";
    if (s === "warn") return "关注";
    if (s === "skip") return "跳过";
    return s;
  };

  const siteScoreRows = report
    ? Object.entries(report.site_scores ?? {}).map(([id, score]) => ({
        site_id: id,
        name: siteDisplayName(sites, id),
        score,
      }))
    : [];

  const findCheckForFinding = (f: PostureReport["findings"][0]): PostureCheck | null => {
    const list = report?.checks ?? [];
    const sid = f.site_id;
    if (!sid) return null;
    if (f.code === "many_disabled_rules") {
      return list.find((c) => c.site_id === sid && c.code === "disabled_rules") ?? null;
    }
    if (f.code === "high_fp_rate") {
      return list.find((c) => c.site_id === sid && c.code === "fp_rate_7d") ?? null;
    }
    if (f.code === "semantic_disabled") {
      return list.find((c) => c.site_id === sid && c.code === "semantic_enabled") ?? null;
    }
    if (f.code === "core_module_off") {
      return list.find((c) => c.site_id === sid && c.category === "module" && c.status === "fail") ?? null;
    }
    return null;
  };

  return (
    <>
      <NervePageHeader
        title="防护巡检"
        subtitle="评估各检测引擎与策略配置：模块覆盖、规则禁用、语义增强与误报趋势"
      />
      <div style={{ display: "flex", gap: 12, marginBottom: 16, flexWrap: "wrap" }}>
        <NerveSelect
          value={siteFilter}
          onChange={(e) => setSiteFilter(e.target.value)}
          options={[
            { value: "", label: "全部应用" },
            ...sites.map((s) => ({
              value: s.id,
              label: `${siteDisplayName(sites, s.id)} (${s.id})`,
            })),
          ]}
          style={{ minWidth: 200 }}
        />
        <NerveButton variant="primary" disabled={scanning} onClick={() => void runScan()}>
          {scanning ? "巡检中…" : "立即巡检"}
        </NerveButton>
        <NerveButton onClick={() => void load()}>刷新</NerveButton>
      </div>

      {loading ? (
        <p className="muted">加载中…</p>
      ) : loadError ? (
        <NerveAlert tone="error">
          {loadError}
          {loadError.includes("404") ? (
            <p className="small" style={{ marginTop: 8 }}>
              服务接口不可用，请确认 WAF 控制面已启动并完成升级。
            </p>
          ) : null}
        </NerveAlert>
      ) : !report ? (
        <p className="muted">暂无数据</p>
      ) : (
        <>
          <div className="kpi-row">
            <div className="kpi-card">
              <div className="kpi-icon kpi-blue">◉</div>
              <div className="kpi-body">
                <div className="kpi-label">综合评分</div>
                <div className="kpi-value">{report.overall_score}</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-orange">!</div>
              <div className="kpi-body">
                <div className="kpi-label">发现问题</div>
                <div className="kpi-value">{report.summary.findings_count}</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-red">⊘</div>
              <div className="kpi-body">
                <div className="kpi-label">关闭模块</div>
                <div className="kpi-value">{report.summary.modules_off}</div>
              </div>
            </div>
            <div className="kpi-card">
              <div className="kpi-icon kpi-green">✓</div>
              <div className="kpi-body">
                <div className="kpi-label">巡检通过项</div>
                <div className="kpi-value">
                  {report.summary.checks_pass ?? "—"}
                  {report.summary.checks_count != null ? (
                    <span className="muted small"> / {report.summary.checks_count}</span>
                  ) : null}
                </div>
              </div>
            </div>
          </div>

          <NerveAlert tone="info" style={{ marginBottom: 16 }}>
            {report.scanned_at ? (
              <span>
                最近评估：<code>{formatTs(report.scanned_at)}</code>
                {report.window_days ? ` · 近 ${report.window_days} 天数据` : null}
              </span>
            ) : (
              "点击「立即巡检」开始评估"
            )}
          </NerveAlert>

          <NerveCard title="各应用评分" style={{ marginBottom: 16 }}>
            <NerveTable
              columns={[
                { key: "name", title: "应用" },
                { key: "site_id", title: "ID", render: (r) => <code>{r.site_id}</code> },
                {
                  key: "score",
                  title: "得分",
                  render: (r) => (
                    <NerveTag tone={r.score >= 90 ? "green" : r.score >= 70 ? "orange" : "red"}>
                      {r.score}
                    </NerveTag>
                  ),
                },
              ]}
              dataSource={siteScoreRows}
              rowKey="site_id"
              emptyText="无应用"
            />
          </NerveCard>

          <NerveCard title="巡检明细（逐项结果）" style={{ marginBottom: 16 }}>
            {(report.checks?.length ?? 0) > 0 ? (
              <NerveTable
                columns={[
                  {
                    key: "status",
                    title: "结果",
                    render: (r: PostureCheck) => (
                      <NerveTag tone={checkTone(r.status)}>{checkLabel(r.status)}</NerveTag>
                    ),
                  },
                  {
                    key: "site_id",
                    title: "应用",
                    render: (r: PostureCheck) => siteDisplayName(sites, r.site_id),
                  },
                  { key: "label", title: "检查项" },
                  {
                    key: "detail",
                    title: "详情",
                    render: (r: PostureCheck) => (
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                        <span>{r.detail}</span>
                        {(r.items?.length ?? 0) > 0 || r.status === "warn" || r.status === "fail" ? (
                          <NerveButton size="sm" variant="link" onClick={() => setDetailCheck(r)}>
                            查看
                          </NerveButton>
                        ) : null}
                      </span>
                    ),
                  },
                ]}
                dataSource={report.checks ?? []}
                rowKey={(r: PostureCheck) => `${r.site_id}-${r.code}`}
                emptyText="无"
              />
            ) : (
              <p className="muted small">暂无逐项明细，请重新执行巡检。</p>
            )}
          </NerveCard>

          <NerveCard title={report.findings.length > 0 ? "需处理问题" : "巡检结论"}>
            <NerveTable
              columns={[
                {
                  key: "severity",
                  title: "级别",
                  render: (r) => <NerveTag tone={severityTone(r.severity)}>{r.severity}</NerveTag>,
                },
                { key: "message", title: "说明" },
                {
                  key: "site_id",
                  title: "应用",
                  render: (r) => (r.site_id ? siteDisplayName(sites, r.site_id) : "—"),
                },
                { key: "recommendation", title: "建议" },
                {
                  key: "action",
                  title: "",
                  render: (r) => {
                    const linked = findCheckForFinding(r);
                    return linked ? (
                      <NerveButton size="sm" variant="link" onClick={() => setDetailCheck(linked)}>
                        查看明细
                      </NerveButton>
                    ) : null;
                  },
                },
              ]}
              dataSource={report.findings}
              rowKey={(r, i) => `${r.code}-${i}`}
              emptyText="所有检查项均已通过或跳过，防护配置健康"
            />
          </NerveCard>

          <PostureCheckDetailModal
            open={detailCheck != null}
            check={detailCheck}
            sites={sites}
            onClose={() => setDetailCheck(null)}
          />
        </>
      )}
    </>
  );
}
