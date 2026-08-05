import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NerveTable,
  NerveTag,
} from "@nerve/ui";
import { apiGet } from "../api";
import { formatTs } from "../lib/datetime";
import { useToast } from "../hooks/useToast";

type AiHealth = {
  ok: boolean;
  checked_at?: string;
  database_configured?: boolean;
  schema?: {
    ok?: boolean;
    tables?: Record<string, { exists: boolean }>;
  };
  engram?: {
    configured?: boolean;
    reachable?: boolean;
  };
};

type EngramStatus = {
  gateway?: { configured?: boolean; reachable?: boolean };
  registry?: { ok?: boolean };
};

type AuditRecord = {
  action?: string;
  actor?: string;
  ip?: string;
  event_id?: string;
  created_at?: string;
};

const TABLE_LABELS: Record<string, string> = {
  waf_ml_labels: "样本标注库",
  waf_posture_scan: "巡检记录",
  waf_fp_remediation: "误报审计",
};

function overallTone(health: AiHealth | null): "green" | "orange" | "red" {
  if (!health) return "orange";
  const schemaOk = health.schema?.ok ?? !health.database_configured;
  if (!schemaOk) return "orange";
  if (health.engram?.configured && !health.engram.reachable) return "orange";
  return "green";
}

function overallLabel(health: AiHealth | null): string {
  return overallTone(health) === "green" ? "就绪" : "部分能力未就绪";
}

function engramLabel(health: AiHealth | null, engram: EngramStatus | null): string {
  const configured =
    health?.engram?.configured ?? engram?.gateway?.configured ?? false;
  if (!configured) return "未启用";
  const reachable = health?.engram?.reachable ?? engram?.gateway?.reachable ?? false;
  return reachable ? "运行中" : "未连接";
}

function statusTone(value: string): "green" | "orange" | "default" {
  if (value === "就绪" || value === "已启用" || value === "运行中" || value === "已加载") {
    return "green";
  }
  if (value.startsWith("待") || value === "未连接" || value === "未配置") {
    return "orange";
  }
  return "default";
}

export function AiOpsPanel() {
  const toast = useToast();
  const [health, setHealth] = useState<AiHealth | null>(null);
  const [engram, setEngram] = useState<EngramStatus | null>(null);
  const [audits, setAudits] = useState<AuditRecord[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [h, e, a] = await Promise.all([
        apiGet<AiHealth>("/api/ai/health"),
        apiGet<EngramStatus>("/api/ai/engram/status"),
        apiGet<{ records: AuditRecord[] }>("/api/ai/audit/recent?limit=30"),
      ]);
      setHealth(h);
      setEngram(e);
      setAudits(a.records ?? []);
    } catch {
      toast("状态加载失败", false);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  if (loading && !health) {
    return <p className="muted">加载中…</p>;
  }

  const schemaOk = health?.schema?.ok ?? !health?.database_configured;
  const missingTables = Object.entries(health?.schema?.tables ?? {})
    .filter(([, v]) => !v.exists)
    .map(([name]) => TABLE_LABELS[name] ?? name);

  const sampleLibValue = schemaOk
    ? "就绪"
    : missingTables.length
      ? `待初始化`
      : "待初始化";

  const statusItems = [
    {
      label: "数据持久化",
      value: health?.database_configured ? "已启用" : "本地模式",
    },
    { label: "学习样本库", value: sampleLibValue },
    { label: "模型推理", value: engramLabel(health, engram) },
    { label: "模型注册表", value: engram?.registry?.ok ? "已加载" : "未配置" },
  ];

  return (
    <div className="ai-pl-page">
      <div className="ai-pl-toolbar">
        <div className="ai-pl-result-meta" style={{ marginBottom: 0 }}>
          <NerveTag tone={overallTone(health)}>{overallLabel(health)}</NerveTag>
          {health?.checked_at ? (
            <span className="muted small">更新于 {formatTs(health.checked_at)}</span>
          ) : null}
        </div>
        <div className="ai-pl-toolbar-actions">
          <NerveButton disabled={loading} onClick={() => void load()}>
            {loading ? "刷新中…" : "刷新"}
          </NerveButton>
        </div>
      </div>

      <NerveCard title="组件状态">
        <div className="ai-pl-status-grid">
          {statusItems.map((item) => (
            <div key={item.label} className="ai-pl-status-item">
              <span className="ai-pl-status-item-label">{item.label}</span>
              <span className="ai-pl-status-item-value">
                <NerveTag tone={statusTone(item.value)}>{item.value}</NerveTag>
              </span>
            </div>
          ))}
        </div>
        {!schemaOk ? (
          <NerveAlert tone="warning">
            样本库与巡检历史尚未就绪，请联系管理员完成数据库初始化。防护与阻断能力不受影响。
            {missingTables.length > 0 ? (
              <span className="small"> 待初始化：{missingTables.join("、")}</span>
            ) : null}
          </NerveAlert>
        ) : null}
        {health?.engram?.configured && !health.engram.reachable ? (
          <NerveAlert tone="info" style={{ marginTop: schemaOk ? 0 : 12 }}>
            模型推理服务暂不可用，不影响 WAF 拦截与规则防护。
          </NerveAlert>
        ) : null}
      </NerveCard>

      <NerveCard title="误报与处置审计">
        {audits.length === 0 ? (
          <div className="ai-pl-empty-hint">
            <p>暂无审计记录</p>
            <span className="muted">误报确认、策略变更等操作将在此展示</span>
          </div>
        ) : (
          <NerveTable
            columns={[
              {
                key: "created_at",
                title: "时间",
                render: (r: AuditRecord) => (r.created_at ? formatTs(r.created_at) : "—"),
              },
              { key: "action", title: "操作" },
              { key: "actor", title: "用户" },
              { key: "ip", title: "IP" },
            ]}
            dataSource={audits}
            rowKey={(r: AuditRecord, i: number) => `${r.created_at ?? i}-${r.action ?? ""}`}
            emptyText="暂无记录"
          />
        )}
      </NerveCard>
    </div>
  );
}
