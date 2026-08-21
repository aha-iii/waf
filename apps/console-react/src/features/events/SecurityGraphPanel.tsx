/** events feature: Security Graph Panel. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NerveInput,
  NerveTable,
} from "@nerve/ui";
import { apiGet, apiPost } from "../../api";
import { useToast } from "../../hooks/useToast";

type GraphResponse = {
  ok: boolean;
  event?: { id: string; src_ip?: string; uri?: string; category: string };
  flows: Record<string, unknown>[];
  graph: {
    nodes: { id: string; label: string; kind: string }[];
    edges: { source: string; target: string; direction: string }[];
  };
};

type Props = {
  /** Auto-load when eventId is present */
  eventId?: string;
  srcIp?: string;
  /** When embedded in event detail, hide the reverse-IP lookup card */
  compact?: boolean;
};

export function SecurityGraphPanel({ eventId = "", srcIp: initialSrcIp = "", compact = false }: Props) {
  const toast = useToast();
  const [eventIdInput, setEventIdInput] = useState(eventId);
  const [srcIp, setSrcIp] = useState(initialSrcIp);
  const [data, setData] = useState<GraphResponse | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setEventIdInput(eventId);
    setSrcIp(initialSrcIp);
  }, [eventId, initialSrcIp]);

  const loadByEvent = useCallback(
    async (id?: string) => {
      const eid = (id ?? eventIdInput).trim();
      if (!eid) {
        toast("请输入事件 ID", false);
        return;
      }
      setLoading(true);
      try {
        const res = await apiGet<GraphResponse>(
          `/api/security-graph/event/${encodeURIComponent(eid)}?window_minutes=30`,
        );
        setData(res);
      } catch (e) {
        toast(String(e), false);
        setData(null);
      } finally {
        setLoading(false);
      }
    },
    [eventIdInput, toast],
  );

  const loadByIp = useCallback(async () => {
    if (!srcIp.trim()) {
      toast("请输入源 IP", false);
      return;
    }
    setLoading(true);
    try {
      const res = await apiGet<GraphResponse>(
        `/api/security-graph/ip/${encodeURIComponent(srcIp.trim())}?minutes=60`,
      );
      setData(res);
    } catch (e) {
      toast(String(e), false);
      setData(null);
    } finally {
      setLoading(false);
    }
  }, [srcIp, toast]);

  useEffect(() => {
    const eid = eventId.trim();
    if (!eid) return;
    let cancelled = false;
    void (async () => {
      setLoading(true);
      try {
        const res = await apiGet<GraphResponse>(
          `/api/security-graph/event/${encodeURIComponent(eid)}?window_minutes=30`,
        );
        if (!cancelled) setData(res);
      } catch (e) {
        if (!cancelled) {
          toast(String(e), false);
          setData(null);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [eventId, toast]);

  const blockIp = async () => {
    const ip = data?.event?.src_ip ?? srcIp.trim();
    if (!ip) {
      toast("无可用 IP", false);
      return;
    }
    try {
      await apiPost("/api/security-graph/block-ip", {
        ip,
        event_id: eventIdInput.trim() || eventId.trim() || undefined,
        reason: "安全图谱一键封禁",
      });
      toast(`已封禁 ${ip}`, true);
    } catch (e) {
      toast(String(e), false);
    }
  };

  return (
    <>
      {!compact ? (
        <div className="dash-grid-2" style={{ marginBottom: 16 }}>
          <NerveCard title="按事件 ID">
            <NerveInput
              value={eventIdInput}
              onChange={(e) => setEventIdInput(e.target.value)}
              placeholder="攻击日志事件 UUID"
              wide
            />
            <div style={{ marginTop: 12 }}>
              <NerveButton variant="primary" loading={loading} onClick={() => void loadByEvent()}>
                查看攻击路径
              </NerveButton>
            </div>
          </NerveCard>
          <NerveCard title="按源 IP 反查">
            <NerveInput
              value={srcIp}
              onChange={(e) => setSrcIp(e.target.value)}
              placeholder="例如 203.0.113.10"
              wide
            />
            <div style={{ marginTop: 12 }}>
              <NerveButton loading={loading} onClick={() => void loadByIp()}>
                反查 Flow
              </NerveButton>
            </div>
          </NerveCard>
        </div>
      ) : loading && !data ? (
        <p className="muted small">加载攻击路径…</p>
      ) : null}

      {!compact && !data && !loading ? (
        <NerveAlert tone="info">
          输入事件 ID 或源 IP 开始关联分析。需 Agent 上报 Flow（可选能力）；无 Flow 时节点为空属正常。
        </NerveAlert>
      ) : null}

      {compact && !loading && !data ? (
        <NerveAlert tone="info">
          暂无攻击路径数据。安全图谱依赖 Agent / Flow 上报（可选能力）；无 Agent 时 WAF 事件研判仍可用。
          部署 Agent 后可在 Platform 或 Observer 查看拓扑，此处展示与事件关联的 Flow。
        </NerveAlert>
      ) : null}

      {data ? (
        <>
          <NerveCard title="拓扑节点">
            <NerveTable
              columns={[
                { key: "kind", title: "类型", render: (r) => <code>{r.kind}</code> },
                { key: "label", title: "标签" },
                { key: "id", title: "ID", render: (r) => <code className="small">{r.id}</code> },
              ]}
              dataSource={data.graph?.nodes ?? []}
              rowKey="id"
              emptyText="暂无节点（无 Flow 数据）"
            />
          </NerveCard>
          <NerveCard title="关联 Flow" style={{ marginTop: 16 }}>
            <NerveTable
              columns={[
                { key: "direction", title: "方向" },
                { key: "src_ip", title: "源 IP" },
                { key: "dst_ip", title: "目标" },
                { key: "pod", title: "Pod" },
                { key: "ts", title: "时间" },
              ]}
              dataSource={
                data.flows as {
                  direction?: string;
                  src_ip?: string;
                  dst_ip?: string;
                  pod?: string;
                  ts?: string;
                }[]
              }
              rowKey={(_, i) => String(i)}
              emptyText="暂无 Flow"
            />
          </NerveCard>
          {(data.event?.src_ip || srcIp) && !compact ? (
            <div style={{ marginTop: 16 }}>
              <NerveButton variant="danger" onClick={() => void blockIp()}>
                封禁此 IP
              </NerveButton>
            </div>
          ) : null}
        </>
      ) : null}
    </>
  );
}
