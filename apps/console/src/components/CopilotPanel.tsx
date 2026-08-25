/** Shared console UI component: Copilot Panel. */
import { useState } from "react";
import { NerveButton, NerveCard, NerveInput, NerveTag } from "@nerve/ui";
import { apiGet, apiPost } from "../api";
import { useToast } from "../hooks/useToast";

type Props = {
  open: boolean;
  onClose: () => void;
  defaultEventId?: string;
};

type CopilotView =
  | { kind: "idle" }
  | { kind: "posture"; score: number; findings: number; high: number }
  | { kind: "fp"; eventId: string; suggestions: string[] }
  | { kind: "incident"; incidentId: string; summary: string; actions: string[]; sections: number }
  | { kind: "raw"; title: string; json: string };

function parsePosture(data: Record<string, unknown>): CopilotView {
  const findings = Array.isArray(data.findings) ? data.findings : [];
  const high = findings.filter(
    (f) => (f as { severity?: string }).severity === "high",
  ).length;
  return {
    kind: "posture",
    score: Number(data.overall_score ?? 0),
    findings: findings.length,
    high,
  };
}

function parseFp(data: Record<string, unknown>, eventId: string): CopilotView {
  const suggestions: string[] = [];
  if (Array.isArray(data.suggestions)) {
    for (const s of data.suggestions) {
      if (typeof s === "string") suggestions.push(s);
      else if (s && typeof s === "object" && "message" in s) {
        suggestions.push(String((s as { message: string }).message));
      }
    }
  }
  if (Array.isArray(data.recommended_actions)) {
    suggestions.push(...(data.recommended_actions as string[]));
  }
  return { kind: "fp", eventId, suggestions: suggestions.slice(0, 8) };
}

function parseIncident(data: Record<string, unknown>): CopilotView {
  const sections = Array.isArray(data.sections) ? data.sections.length : 0;
  const actions = Array.isArray(data.suggested_actions)
    ? (data.suggested_actions as string[]).slice(0, 6)
    : [];
  const triageSec = Array.isArray(data.sections)
    ? (data.sections as { section?: string; data?: { summary?: string } }[]).find(
        (s) => s.section === "triage",
      )
    : undefined;
  const summary =
    triageSec?.data?.summary ??
    String(data.event_id ?? "incident");
  return {
    kind: "incident",
    incidentId: String(data.incident_id ?? ""),
    summary,
    actions,
    sections,
  };
}

export function CopilotPanel({ open, onClose, defaultEventId = "" }: Props) {
  const toast = useToast();
  const [eventId, setEventId] = useState(defaultEventId);
  const [busy, setBusy] = useState(false);
  const [view, setView] = useState<CopilotView>({ kind: "idle" });

  if (!open) return null;

  const run = async (
    label: string,
    fn: () => Promise<unknown>,
    mapView: (data: Record<string, unknown>) => CopilotView,
  ) => {
    setBusy(true);
    setView({ kind: "idle" });
    try {
      const res = (await fn()) as Record<string, unknown>;
      if (res.ok === false) {
        throw new Error(String(res.message ?? res.error ?? "请求失败"));
      }
      setView(mapView(res));
      toast(`${label} 完成`, true);
    } catch (e) {
      setView({ kind: "raw", title: label, json: String(e) });
      toast(String(e), false);
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="copilot-drawer" aria-label="Thalamus Copilot">
      <div className="copilot-drawer-head">
        <strong>Thalamus Copilot</strong>
        <button type="button" className="ant-btn ant-btn-sm" onClick={onClose}>
          关闭
        </button>
      </div>
      <p className="muted small">只读建议与预演；封禁/改策略需 Console 确认。</p>
      <label className="small">事件 ID</label>
      <NerveInput
        className="cloud-login-input wide"
        value={eventId}
        onChange={(e) => setEventId(e.target.value)}
        placeholder="evt-..."
      />
      <div className="sync-actions" style={{ marginTop: 12 }}>
        <NerveButton
          disabled={busy}
          onClick={() =>
            void run("防护巡检", () => apiGet("/api/posture/scan"), parsePosture)
          }
        >
          巡检
        </NerveButton>
        <NerveButton
          disabled={busy || !eventId.trim()}
          onClick={() =>
            void run(
              "误报 Copilot",
              () => apiGet(`/api/events/${encodeURIComponent(eventId.trim())}/fp-suggestions`),
              (d) => parseFp(d, eventId.trim()),
            )
          }
        >
          误报
        </NerveButton>
        <NerveButton
          disabled={busy || !eventId.trim()}
          onClick={() =>
            void run(
              "SOC 报告",
              () =>
                apiPost("/api/incident/report", {
                  event_id: eventId.trim(),
                  include_posture: true,
                  include_graph: true,
                }),
              parseIncident,
            )
          }
        >
          Incident
        </NerveButton>
      </div>

      <div className="copilot-results">
        {view.kind === "idle" ? (
          <p className="muted small">选择操作查看结构化结果</p>
        ) : null}
        {view.kind === "posture" ? (
          <NerveCard title="防护巡检">
            <p>
              综合分 <strong>{view.score}</strong>
              {view.score < 70 ? <NerveTag color="orange">需关注</NerveTag> : null}
            </p>
            <p className="muted small">
              发现 {view.findings} 条 · 高危 {view.high} 条
            </p>
          </NerveCard>
        ) : null}
        {view.kind === "fp" ? (
          <NerveCard title="误报 Copilot">
            <p className="muted small">事件 {view.eventId}</p>
            <ul>
              {view.suggestions.length ? (
                view.suggestions.map((s) => <li key={s}>{s}</li>)
              ) : (
                <li>暂无自动建议，请人工研判</li>
              )}
            </ul>
            <p className="muted small">应用修复需在研判弹窗 confirm-fp</p>
          </NerveCard>
        ) : null}
        {view.kind === "incident" ? (
          <NerveCard title="SOC 报告">
            <p className="muted small">{view.incidentId}</p>
            <p>{view.summary}</p>
            <p className="muted small">{view.sections} 个分析段落</p>
            <h4 className="small">建议动作</h4>
            <ul>
              {view.actions.map((a) => (
                <li key={a}>{a}</li>
              ))}
            </ul>
          </NerveCard>
        ) : null}
        {view.kind === "raw" ? (
          <NerveCard title={view.title}>
            <pre className="json-view">{view.json}</pre>
          </NerveCard>
        ) : null}
      </div>
    </aside>
  );
}
