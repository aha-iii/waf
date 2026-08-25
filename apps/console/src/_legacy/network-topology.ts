/** Legacy console module: Network topology. */
import { esc } from "./components/ui.js";

export type TopologyNode = {
  id: string;
  label: string;
  kind: string;
  namespace?: string;
};

export type TopologyEdge = {
  source: string;
  target: string;
  direction: string;
  count: number;
};

export type TopologyGraph = {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  stats?: {
    total_flows: number;
    north_south: number;
    east_west: number;
    agents: number;
  };
};

const KIND_COLOR: Record<string, string> = {
  workload: "#1677ff",
  pod: "#52c41a",
  ip: "#722ed1",
  external: "#fa541c",
  node: "#13c2c2",
  unknown: "#8c8c8c",
};

/** Trim by direction and TopN; keep at most maxPerSide high-traffic nodes on each of Workloads / Services sides */
export function pruneServiceMapGraph(
  graph: TopologyGraph,
  direction: "all" | "east_west" | "north_south",
  topEdges = 30,
  maxPerSide = 14,
): TopologyGraph {
  let edges = graph.edges.filter((e) => direction === "all" || e.direction === direction);
  edges = [...edges].sort((a, b) => b.count - a.count).slice(0, topEdges);
  const nodeIds = new Set<string>();
  for (const e of edges) {
    nodeIds.add(e.source);
    nodeIds.add(e.target);
  }
  let nodes = graph.nodes.filter((n) => nodeIds.has(n.id));

  const traffic = new Map<string, number>();
  for (const e of edges) {
    traffic.set(e.source, (traffic.get(e.source) ?? 0) + e.count);
    traffic.set(e.target, (traffic.get(e.target) ?? 0) + e.count);
  }
  const score = (id: string): number => traffic.get(id) ?? 0;
  const isServiceSide = (n: TopologyNode): boolean => {
    const k = topologyNodeKind(n);
    return k === "service" || k === "external";
  };
  const wl = nodes.filter((n) => !isServiceSide(n)).sort((a, b) => score(b.id) - score(a.id));
  const svc = nodes.filter(isServiceSide).sort((a, b) => score(b.id) - score(a.id));
  const keep = new Set([
    ...wl.slice(0, maxPerSide).map((n) => n.id),
    ...svc.slice(0, maxPerSide).map((n) => n.id),
  ]);
  nodes = nodes.filter((n) => keep.has(n.id));
  edges = edges.filter((e) => keep.has(e.source) && keep.has(e.target));
  return { nodes, edges, stats: graph.stats };
}

export function shortTopologyLabel(n: TopologyNode): string {
  let s = n.label || n.id;
  if (s.startsWith("cgroup:")) s = s.slice(7, 15);
  if (s.startsWith("wl:")) s = s.slice(3);
  if (s.startsWith("ip:")) s = s.slice(3);
  if (s.startsWith("ext:")) s = s.slice(4);
  if (s.length > 24) s = `${s.slice(0, 22)}…`;
  return s;
}

export function topologyNodeKind(n: TopologyNode): "app" | "workload" | "service" | "external" | "infra" | "unknown" {
  if (n.kind === "external" || n.id.startsWith("ext:")) return "external";
  if (n.kind === "ip") return "service";
  if (n.kind === "pod") return "workload";
  if (n.kind === "workload") return "app";
  if (n.kind === "node") return "infra";
  return "unknown";
}

function layoutNodes(nodes: TopologyNode[], w: number, h: number): Map<string, { x: number; y: number }> {
  const pos = new Map<string, { x: number; y: number }>();
  const cx = w / 2;
  const cy = h / 2;
  const r = Math.min(w, h) * 0.38;
  nodes.forEach((n, i) => {
    const angle = (2 * Math.PI * i) / Math.max(nodes.length, 1) - Math.PI / 2;
    pos.set(n.id, { x: cx + r * Math.cos(angle), y: cy + r * Math.sin(angle) });
  });
  return pos;
}

export function renderTopologySvg(graph: TopologyGraph, direction = "all"): string {
  const w = 920;
  const h = 360;
  if (!graph.nodes.length) {
    return `<div class="ant-empty muted" style="padding:48px">暂无拓扑数据（需 Agent flow-relay 上报或实时会话）</div>`;
  }
  const pos = layoutNodes(graph.nodes, w, h);
  const edgeColor = (d: string) => (d === "north_south" ? "#fa541c" : d === "east_west" ? "#1677ff" : "#bfbfbf");

  const lines = graph.edges
    .filter((e) => direction === "all" || e.direction === direction)
    .slice(0, 80)
    .map((e) => {
      const a = pos.get(e.source);
      const b = pos.get(e.target);
      if (!a || !b) return "";
      return `<line x1="${a.x}" y1="${a.y}" x2="${b.x}" y2="${b.y}" stroke="${edgeColor(e.direction)}" stroke-width="${Math.min(4, 1 + Math.log10(e.count + 1))}" opacity="0.65"/>`;
    })
    .join("");

  const circles = graph.nodes
    .map((n) => {
      const p = pos.get(n.id)!;
      const color = KIND_COLOR[n.kind] ?? KIND_COLOR.unknown;
      const label = n.label.length > 18 ? `${n.label.slice(0, 16)}…` : n.label;
      return `<g>
          <circle cx="${p.x}" cy="${p.y}" r="14" fill="${color}" opacity="0.9"/>
          <title>${esc(n.id)} (${esc(n.kind)})</title>
          <text x="${p.x}" y="${p.y + 28}" text-anchor="middle" font-size="10" fill="#595959">${esc(label)}</text>
        </g>`;
    })
    .join("");

  return `<svg viewBox="0 0 ${w} ${h}" class="topology-svg">${lines}${circles}</svg>`;
}

/** Build a simple Service Map from live sessions (fallback when topology store is unavailable) */
export function sessionsToGraph(
  sessions: { pod?: string; dst_ip?: string; dport?: number }[],
): TopologyGraph {
  const nodes = new Map<string, TopologyNode>();
  const edgeMap = new Map<string, TopologyEdge>();

  const ensureNode = (id: string, label: string, kind: string): void => {
    if (!nodes.has(id)) nodes.set(id, { id, label, kind });
  };

  for (const s of sessions) {
    const pod = s.pod?.trim() || "local";
    const srcId = `wl:${pod}`;
    ensureNode(srcId, pod.replace(/^cgroup:/, "wl-").slice(0, 20), "workload");
    const dstIp = s.dst_ip?.trim();
    if (!dstIp) continue;
    const dstId = `ip:${dstIp}`;
    ensureNode(dstId, dstIp, "ip");
    const key = `${srcId}->${dstId}`;
    const prev = edgeMap.get(key);
    if (prev) prev.count += 1;
    else edgeMap.set(key, { source: srcId, target: dstId, direction: "east_west", count: 1 });
  }

  return {
    nodes: [...nodes.values()],
    edges: [...edgeMap.values()],
    stats: { total_flows: sessions.length, north_south: 0, east_west: edgeMap.size, agents: 1 },
  };
}

export function renderTrendBars(trend: { day: string; count: number }[]): string {
  if (!trend.length) return `<div class="muted small">暂无趋势（先「同步入库」积累 L3 事件）</div>`;
  const max = Math.max(...trend.map((t) => t.count), 1);
  return `<div class="net-trend-wrap">${trend
    .map((t) => {
      const h = Math.max(8, Math.round((t.count / max) * 100));
      return `<div class="net-trend-item" title="${esc(t.day)}: ${t.count}">
        <div class="net-trend-bar" style="height:${h}%"></div>
        <div class="net-trend-lbl">${esc(t.day.slice(5))}</div>
      </div>`;
    })
    .join("")}</div>`;
}

export function renderTopList(
  title: string,
  items: { label: string; count: number; ip?: string }[],
  empty: string,
): string {
  if (!items.length) return `<div class="net-top-card"><h4>${esc(title)}</h4><p class="muted small">${esc(empty)}</p></div>`;
  const rows = items
    .map(
      (it) => `<div class="net-top-row">
      <span class="net-top-label">${esc(it.label)}</span>
      <span class="net-top-count">${it.count}</span>
      ${it.ip ? `<span class="net-top-actions" data-ip="${esc(it.ip)}"></span>` : ""}
    </div>`,
    )
    .join("");
  return `<div class="net-top-card"><h4>${esc(title)}</h4>${rows}</div>`;
}
