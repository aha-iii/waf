/** Console page: AI training / sample management. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NervePageHeader,
  NerveTable,
  NerveTag,
} from "@nerve/ui";
import { apiGet } from "../api";
import { formatTs } from "../lib/datetime";
import { useToast } from "../hooks/useToast";

type LabelStats = {
  ok: boolean;
  days?: number;
  total?: number;
  by_type?: { label_type: string; count: number }[];
  stats?: { total: number; by_type: { label_type: string; count: number }[] };
};

function normalizeLabelStats(raw: LabelStats): {
  total: number;
  byType: { label_type: string; count: number }[];
} {
  if (raw.stats) {
    return { total: raw.stats.total, byType: raw.stats.by_type ?? [] };
  }
  return { total: raw.total ?? 0, byType: raw.by_type ?? [] };
}

const LABEL_TYPE_CN: Record<string, string> = {
  false_positive: "误报",
  confirmed_attack: "确认攻击",
  shadow_fn: "Shadow 漏报",
  benign: "正常",
};

const LABEL_TONE: Record<string, "default" | "green" | "red" | "orange" | "blue"> = {
  false_positive: "orange",
  confirmed_attack: "red",
  shadow_fn: "blue",
  benign: "green",
};

type MlLabel = {
  id: string;
  label_type: string;
  source: string;
  site_id?: string;
  uri?: string;
  rule_id?: string;
  created_at?: string;
};

export function AiTrainingPage({ embedded = false }: { embedded?: boolean }) {
  const toast = useToast();
  const [stats, setStats] = useState<LabelStats | null>(null);
  const [labels, setLabels] = useState<MlLabel[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(false);
    try {
      const [st, list] = await Promise.all([
        apiGet<LabelStats>("/api/ai/ml-labels/stats?days=30"),
        apiGet<{ labels: MlLabel[] }>("/api/ai/ml-labels?limit=50"),
      ]);
      setStats(st);
      setLabels(list.labels ?? []);
    } catch {
      setLoadError(true);
      toast("样本库加载失败", false);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const exportLabels = async () => {
    try {
      const data = await apiGet<{ count: number; records: unknown[] }>(
        "/api/ai/ml-labels/export?limit=5000",
      );
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `nerve-ml-labels-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast(`已导出 ${data.count} 条`, true);
    } catch {
      toast("导出失败", false);
    }
  };

  const statView = stats
    ? normalizeLabelStats(stats)
    : { total: 0, byType: [] as { label_type: string; count: number }[] };

  if (loading) {
    return <p className="muted">加载中…</p>;
  }

  return (
    <div className="ai-pl-page">
      {!embedded ? (
        <NervePageHeader title="样本库" subtitle="误报与漏报标注，用于持续学习" />
      ) : null}

      {embedded ? (
        <p className="ai-pl-hint">在攻击日志中确认误报后，样本将自动收录。</p>
      ) : null}

      {loadError ? (
        <NerveAlert tone="warning">
          样本库暂不可用，请确认数据库迁移已完成。防护与阻断能力不受影响。
        </NerveAlert>
      ) : null}

      <div className="kpi-row ai-pl-kpi-row">
        <div className="kpi-card">
          <div className="kpi-icon kpi-blue">Σ</div>
          <div className="kpi-body">
            <div className="kpi-label">近 30 天</div>
            <div className="kpi-value">{statView.total}</div>
          </div>
        </div>
        {statView.byType.map((t) => (
          <div className="kpi-card" key={t.label_type}>
            <div className="kpi-icon kpi-green">◆</div>
            <div className="kpi-body">
              <div className="kpi-label">{LABEL_TYPE_CN[t.label_type] ?? t.label_type}</div>
              <div className="kpi-value">{t.count}</div>
            </div>
          </div>
        ))}
      </div>

      <div className="ai-pl-toolbar">
        <span className="muted small">共 {labels.length} 条最近样本</span>
        <div className="ai-pl-toolbar-actions">
          <NerveButton variant="primary" onClick={() => void exportLabels()}>
            导出标注
          </NerveButton>
          <NerveButton onClick={() => void load()}>刷新</NerveButton>
        </div>
      </div>

      <NerveCard title="最近样本">
        {labels.length === 0 && !loadError ? (
          <div className="ai-pl-empty-hint">
            <p>暂无样本</p>
            <span className="muted">在攻击日志 → 研判详情中确认误报后自动入库</span>
          </div>
        ) : (
          <NerveTable
            columns={[
              {
                key: "label_type",
                title: "类型",
                render: (r) => (
                  <NerveTag tone={LABEL_TONE[r.label_type] ?? "default"}>
                    {LABEL_TYPE_CN[r.label_type] ?? r.label_type}
                  </NerveTag>
                ),
              },
              { key: "source", title: "来源" },
              { key: "site_id", title: "应用" },
              {
                key: "rule_id",
                title: "规则",
                render: (r) => (r.rule_id ? <code>{r.rule_id}</code> : "—"),
              },
              {
                key: "uri",
                title: "URI",
                render: (r) => (
                  <span className="small">
                    {r.uri ? (r.uri.length > 48 ? `${r.uri.slice(0, 48)}…` : r.uri) : "—"}
                  </span>
                ),
              },
              {
                key: "created_at",
                title: "时间",
                render: (r) => (r.created_at ? formatTs(r.created_at) : "—"),
              },
            ]}
            dataSource={labels}
            rowKey="id"
            emptyText="暂无样本"
          />
        )}
      </NerveCard>
    </div>
  );
}
