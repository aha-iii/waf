/** Legacy (pre-React) console module: Network view. */
import type { WafEvent } from "./security-view.js";
import { apiBaselineAllow, apiGet, apiNetworkGeoip, apiPost, apiTempAllow, apiTempDeny } from "./api.js";
import {
  actionTag,
  bindPaginationBar,
  btn,
  card,
  categoryLabel,
  defaultPaginationState,
  el,
  esc,
  filterSelect,
  mountPaginatedTable,
  pageHeader,
  renderPaginatedSection,
  runMutatingAction,
  tabBar,
  searchInput,
  tableToolbar,
  tag,
  type PaginationState,
} from "./components/ui.js";
import { PAGE_META } from "./nav.js";
import {
  destroyServiceMap,
  mountServiceMap,
  serviceMapShellHtml,
  updateServiceMap,
} from "./network-topology-mount.js";
import {
  renderTopList,
  renderTrendBars,
  sessionsToGraph,
  type TopologyGraph,
} from "./network-topology.js";

/** Agent live observation (via 8780 → NERVE_URL proxy) */
type NerveCounts = Record<string, number>;

type FlowSession = {
  id: string;
  state: string;
  pod?: string;
  pid?: number;
  dst_ip?: string;
  dport?: number;
  duration_ms?: number;
  policy_hits?: number;
  risk_score?: number;
  anomaly_count?: number;
  connect_time?: string;
};

type NerveAnomaly = {
  type: string;
  severity?: string;
  pod?: string;
  src_ip?: string;
  dst_ip?: string;
  dport?: number;
  access_addr?: string;
  message?: string;
  action?: string;
  risk_score?: number;
  explain?: string;
  session_id?: string;
  time?: string;
  signals?: string[];
};

type PolicyHit = {
  src_ip: string;
  dport?: number;
  action?: string;
  pkt_len?: number;
  session_id?: string;
  total_hits?: number;
  time?: string;
};

type BaselineEntry = {
  pod: string;
  phase: string;
  destinations?: string[];
  violations?: number;
};

type NerveLiveState = {
  counts?: NerveCounts;
  sessions?: { open?: FlowSession[]; closed?: FlowSession[] };
  anomalies?: NerveAnomaly[];
  policy_hits?: PolicyHit[];
  baseline?: BaselineEntry[];
  status?: Record<string, unknown>;
  error?: string;
};

type SessionGraph = {
  session?: FlowSession;
  flows?: Record<string, unknown>[];
  anomalies?: NerveAnomaly[];
  policy_hits?: PolicyHit[];
};

let pollTimer: ReturnType<typeof setInterval> | null = null;
const geoCache = new Map<string, string>();

function stopPolling(): void {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

function workloadLabel(pod?: string): string {
  if (!pod) return "—";
  if (pod.startsWith("pod:")) return pod.slice(4);
  if (pod.startsWith("container:")) return `容器 ${pod.slice(10)}`;
  if (pod.startsWith("cgroup:")) return `工作负载 ${pod.slice(7, 15)}…`;
  return pod.length > 24 ? `${pod.slice(0, 22)}…` : pod;
}

function sessionStateTag(state: string): string {
  const map: Record<string, "green" | "orange" | "red" | "blue"> = {
    open: "green",
    closed: "blue",
    orphan: "orange",
  };
  const labels: Record<string, string> = { open: "活跃", closed: "已关闭", orphan: "孤儿" };
  return tag(labels[state] ?? state, map[state] ?? "blue");
}

function anomalyTypeLabel(t: string): string {
  return categoryLabel(t);
}

function governanceActionTag(action?: string): string {
  if (!action) return tag("仅记录", "orange");
  if (action.includes("deny")) return tag("L1 已封禁", "red");
  if (action.includes("alert")) return tag("L2 告警", "orange");
  return tag(action, "blue");
}

function isGovernanceAnomaly(a: NerveAnomaly): boolean {
  if (a.action?.includes("deny")) return true;
  if (a.type === "baseline_violation" || a.type === "escalation" || a.type === "port_scan") return true;
  if ((a.risk_score ?? 0) >= 40) return true;
  if (a.signals?.length) return true;
  return false;
}

function formatTime(iso?: string): string {
  if (!iso) return "—";
  return iso.slice(0, 19).replace("T", " ");
}

async function geoLabel(ip?: string): Promise<string> {
  if (!ip || ip === "—") return "—";
  const cached = geoCache.get(ip);
  if (cached) return cached;
  try {
    const g = await apiNetworkGeoip(ip);
    const label = g.country_label ?? g.country ?? "—";
    geoCache.set(ip, label);
    return label;
  } catch {
    return "—";
  }
}

function ipActionButtons(ip?: string, pod?: string): string {
  if (!ip || ip === "—" || ip.includes(":")) return "";
  const podAttr = pod ? ` data-pod="${esc(pod)}"` : "";
  return `<span class="net-action-btns">
    ${btn({ label: "封禁", size: "sm", variant: "danger", className: "btn-net-deny", attrs: { "data-ip": ip } })}
    ${btn({ label: "解封", size: "sm", className: "btn-net-allow", attrs: { "data-ip": ip } })}
    ${pod ? btn({ label: "基线加白", size: "sm", className: "btn-net-bl-allow", attrs: { "data-ip": ip, "data-pod": pod } }) : ""}
  </span>`;
}

function bindIpActions(container: ParentNode, toast: (m: string, ok?: boolean) => void): void {
  container.querySelectorAll(".btn-net-deny").forEach((b) => {
    b.addEventListener("click", () => {
      const ip = (b as HTMLElement).dataset.ip!;
      void runMutatingAction({
        action: "save",
        subject: `封禁 ${ip}`,
        confirmMessage: `L1 应急封禁 IP ${ip}（写入 Agent deny map）？`,
        successMessage: "已封禁",
        run: async () => {
          await apiTempDeny(ip, "network-console");
        },
      }).then(() => toast(`已封禁 ${ip}`, true)).catch((e) => toast(String(e), false));
    });
  });
  container.querySelectorAll(".btn-net-allow").forEach((b) => {
    b.addEventListener("click", () => {
      const ip = (b as HTMLElement).dataset.ip!;
      void runMutatingAction({
        action: "save",
        subject: `解封 ${ip}`,
        confirmMessage: `解封 IP ${ip}？`,
        successMessage: "已解封",
        run: async () => {
          await apiTempAllow(ip);
        },
      }).then(() => toast(`已解封 ${ip}`, true)).catch((e) => toast(String(e), false));
    });
  });
  container.querySelectorAll(".btn-net-bl-allow").forEach((b) => {
    b.addEventListener("click", () => {
      const ip = (b as HTMLElement).dataset.ip!;
      const pod = (b as HTMLElement).dataset.pod!;
      void runMutatingAction({
        action: "save",
        subject: "基线加白",
        confirmMessage: `将 ${ip} 加入工作负载 ${workloadLabel(pod)} 的基线？`,
        successMessage: "已加白",
        run: async () => {
          await apiBaselineAllow(pod, ip);
        },
      }).then(() => toast(`基线已允许 ${ip}`, true)).catch((e) => toast(String(e), false));
    });
  });
}

function openSessionGraphModal(sessionId: string, toast: (m: string, ok?: boolean) => void): void {
  void apiGet<SessionGraph>(`/api/nerve/observer/session?id=${encodeURIComponent(sessionId)}`)
    .then((g) => {
      const s = g.session;
      const mask = el("div", "ant-modal-mask");
      const wrap = el("div", "ant-modal-wrap");
      const modal = el("div", "ant-modal ant-modal-xl");
      const flows = g.flows ?? [];
      const anomalies = g.anomalies ?? [];
      const hits = g.policy_hits ?? [];
      modal.innerHTML = `
        <div class="ant-modal-header">
          <span>SessionGraph · ${esc(sessionId.slice(0, 12))}…</span>
          <button type="button" class="ant-modal-close">×</button>
        </div>
        <div class="ant-modal-body">
          <div class="kpi-row" style="margin-top:0">
            <div class="kpi-card"><div class="kpi-body">
              <div class="kpi-label">工作负载</div>
              <div class="kpi-value" style="font-size:16px">${esc(workloadLabel(s?.pod))}</div>
              <div class="kpi-sub">${esc(s?.dst_ip ?? "—")}:${s?.dport ?? "—"} · ${sessionStateTag(s?.state ?? "—")}</div>
            </div></div>
            <div class="kpi-card"><div class="kpi-body">
              <div class="kpi-label">会话指标</div>
              <div class="kpi-value" style="font-size:16px">${s?.duration_ms != null ? `${s.duration_ms} ms` : "—"}</div>
              <div class="kpi-sub">流 ${flows.length} · 异常 ${anomalies.length} · 策略命中 ${hits.length}</div>
            </div></div>
          </div>
          <h4 class="net-section-title">相关连接流</h4>
          <div class="ant-table-wrapper"><table class="ant-table ant-table-sm">
            <thead><tr><th>类型</th><th>目标</th><th>端口</th><th>时间</th></tr></thead>
            <tbody>${flows.length ? flows.map((f) => `<tr>
              <td><code>${esc(String(f.type ?? "—"))}</code></td>
              <td><code>${esc(String(f.dst_ip ?? "—"))}</code></td>
              <td>${esc(String(f.dport ?? "—"))}</td>
              <td class="small muted">${esc(formatTime(String(f.time ?? "")))}</td>
            </tr>`).join("") : `<tr><td colspan="4" class="muted">暂无</td></tr>`}
            </tbody></table></div>
          <h4 class="net-section-title">异常与治理</h4>
          <div class="ant-table-wrapper"><table class="ant-table ant-table-sm">
            <thead><tr><th>类型</th><th>动作</th><th>风险</th><th>说明</th></tr></thead>
            <tbody>${anomalies.length ? anomalies.map((a) => `<tr>
              <td>${anomalyTypeLabel(a.type)}</td>
              <td>${governanceActionTag(a.action)}</td>
              <td>${a.risk_score ?? "—"}</td>
              <td class="small">${esc((a.message ?? a.explain ?? "—").slice(0, 120))}</td>
            </tr>`).join("") : `<tr><td colspan="4" class="muted">暂无</td></tr>`}
            </tbody></table></div>
          <h4 class="net-section-title">TC 策略命中</h4>
          <div class="ant-table-wrapper"><table class="ant-table ant-table-sm">
            <thead><tr><th>源 IP</th><th>包长</th><th>累计</th><th>时间</th></tr></thead>
            <tbody>${hits.length ? hits.map((h) => `<tr>
              <td><code>${esc(h.src_ip)}</code></td>
              <td>${h.pkt_len ?? "—"} B</td>
              <td>${h.total_hits ?? "—"}</td>
              <td class="small muted">${esc(formatTime(h.time))}</td>
            </tr>`).join("") : `<tr><td colspan="4" class="muted">暂无</td></tr>`}
            </tbody></table></div>
        </div>
        <div class="ant-modal-footer">
          <button type="button" class="ant-btn" id="sg-close">关闭</button>
        </div>`;
      wrap.appendChild(modal);
      mask.appendChild(wrap);
      document.body.appendChild(mask);
      const close = (): void => mask.remove();
      modal.querySelector(".ant-modal-close")?.addEventListener("click", close);
      modal.querySelector("#sg-close")?.addEventListener("click", close);
      mask.addEventListener("click", (e) => {
        if (e.target === mask) close();
      });
    })
    .catch((e) => toast(String(e), false));
}

const NETWORK_CATEGORY_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "orphan_connect", label: "孤儿连接" },
  { value: "port_scan", label: "端口扫描" },
  { value: "baseline_violation", label: "基线违规" },
  { value: "escalation", label: "升级封禁" },
  { value: "anomaly", label: "异常行为" },
  { value: "l3_policy", label: "L3 策略" },
];

export async function renderNetworkEvents(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  stopPolling();
  const meta = PAGE_META.network;
  root.innerHTML = `
    <div id="net-live-root">
    ${pageHeader(
      meta.title,
      meta.subtitle,
      `<span id="net-agent-badge" class="ant-tag ant-tag-default">检测 Agent…</span>`,
    )}
    ${tableToolbar(
      btn({ id: "btn-net-pause", label: "暂停刷新" }),
      btn({ id: "btn-net-poll-db", label: "同步入库" }),
      btn({ id: "btn-net-refresh", label: "立即刷新", variant: "primary" }),
    )}
    ${tabBar([
      { id: "overview", label: "态势总览", active: true },
      { id: "topology", label: "网络拓扑" },
      { id: "live-sessions", label: "实时会话" },
      { id: "live-anomalies", label: "异常与治理" },
      { id: "live-baseline", label: "行为基线" },
      { id: "live-policy", label: "策略命中" },
      { id: "history", label: "历史归档" },
    ])}
    <div id="net-panel"></div>
    </div>`;

  let live: NerveLiveState = {};
  let paused = false;
  let activeTab = "overview";
  let showLowPriority = false;
  let topoDirection: "all" | "east_west" | "north_south" = "all";
  let topoTopN = 30;
  let topoSearch = "";
  let topoMounted = false;
  let topoSource = "—";

  let cachedEvents: WafEvent[] = [];
  let evBackend = "—";
  let evTotal = 0;
  const evPage: PaginationState = { ...defaultPaginationState() };
  let networkStats: Record<string, unknown> = {};
  let topoGraph: TopologyGraph | null = null;
  const sessionGeo = new Map<string, string>();

  function setAgentBadge(ok: boolean, detail: string): void {
    const el = root.querySelector("#net-agent-badge");
    if (!el) return;
    el.className = ok ? "ant-tag ant-tag-success" : "ant-tag ant-tag-error";
    el.textContent = detail;
  }

  function effectiveTopology(apiGraph: TopologyGraph | null, sessions: FlowSession[]): TopologyGraph {
    const fromSessions = sessionsToGraph(sessions);
    if (!apiGraph?.nodes?.length) return fromSessions;
    return apiGraph;
  }

  function paintCapabilityKpis(container: ParentNode): void {
    const kpis = container.querySelector("#net-capability-kpis");
    if (!kpis) return;
    const c = live.counts ?? {};
    const open = live.sessions?.open?.length ?? 0;
    const baseline = live.baseline ?? [];
    const enforceN = baseline.filter((b) => b.phase === "enforce").length;
    const govN = (live.anomalies ?? []).filter(isGovernanceAnomaly).length;
    const policyOn = (live.status?.policy as { enabled?: boolean } | undefined)?.enabled ?? false;
    const ifaces = ((live.status?.policy as { ifaces?: string[] })?.ifaces ?? []).join(", ") || "未挂载";
    kpis.innerHTML = `
      <div class="kpi-card"><div class="kpi-icon kpi-blue">◎</div><div class="kpi-body">
        <div class="kpi-label">连接观测</div>
        <div class="kpi-value">${(c.connect4 ?? 0) + (c.connect6 ?? 0)}</div>
        <div class="kpi-sub">connect · release ${c.release ?? 0} · 活跃会话 ${open}</div>
      </div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-purple">◇</div><div class="kpi-body">
        <div class="kpi-label">行为基线</div>
        <div class="kpi-value">${baseline.length}</div>
        <div class="kpi-sub">${enforceN} enforce · ${baseline.length - enforceN} learning</div>
      </div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-red">⛨</div><div class="kpi-body">
        <div class="kpi-label">L1 治理</div>
        <div class="kpi-value">${c.policy_hit ?? 0}</div>
        <div class="kpi-sub">${policyOn ? `TC 已挂载 ${ifaces}` : "需 Agent --policy-iface"} · 治理异常 ${govN}</div>
      </div></div>
      <div class="kpi-card"><div class="kpi-icon kpi-orange">⎔</div><div class="kpi-body">
        <div class="kpi-label">SessionGraph</div>
        <div class="kpi-value">${open}</div>
        <div class="kpi-sub">点击会话「诊断」查看流+异常+policy 关联</div>
      </div></div>`;
  }

  function sessionRows(sessions: FlowSession[]): string {
    if (!sessions.length) return `<tr><td colspan="10" class="muted">暂无会话（在另一终端执行 curl 制造出站连接）</td></tr>`;
    return sessions
      .map(
        (s) => `<tr data-dst-ip="${esc(s.dst_ip ?? "")}">
      <td>${sessionStateTag(s.state)}</td>
      <td><span title="${esc(s.pod ?? "")}">${esc(workloadLabel(s.pod))}</span>${s.pid ? `<br><span class="muted small">pid ${s.pid}</span>` : ""}</td>
      <td><code>${esc(s.dst_ip ?? "—")}</code></td>
      <td class="small geo-cell" data-ip="${esc(s.dst_ip ?? "")}">${esc(sessionGeo.get(s.dst_ip ?? "") ?? "…")}</td>
      <td>${s.dport ?? "—"}</td>
      <td>${s.duration_ms != null ? `${s.duration_ms} ms` : "—"}</td>
      <td>${s.risk_score != null ? `<span class="${s.risk_score >= 70 ? "kpi-danger" : ""}">${s.risk_score}</span>` : "—"}</td>
      <td>${s.policy_hits ?? 0}</td>
      <td>${ipActionButtons(s.dst_ip, s.pod)}</td>
      <td>${s.id ? btn({ label: "诊断", size: "sm", className: "btn-sg", attrs: { "data-sid": s.id } }) : "—"}</td>
    </tr>`,
      )
      .join("");
  }

  async function hydrateSessionGeo(panel: ParentNode): Promise<void> {
    const ips = new Set<string>();
    panel.querySelectorAll<HTMLElement>(".geo-cell").forEach((el) => {
      const ip = el.dataset.ip;
      if (ip) ips.add(ip);
    });
    await Promise.all(
      [...ips].map(async (ip) => {
        const label = await geoLabel(ip);
        sessionGeo.set(ip, label);
        panel.querySelectorAll(`.geo-cell[data-ip="${ip}"]`).forEach((el) => {
          el.textContent = label;
        });
      }),
    );
  }

  function anomalyRows(anomalies: NerveAnomaly[]): string {
    const filtered = showLowPriority ? anomalies : anomalies.filter(isGovernanceAnomaly);
    if (!filtered.length) {
      return `<tr><td colspan="7" class="muted">${showLowPriority ? "暂无异常" : "暂无治理级异常（orphan 低优先级已隐藏，可勾选显示）"}</td></tr>`;
    }
    return filtered
      .map(
        (a, idx) => `<tr class="${a.type === "orphan_connect" ? "net-row-muted" : ""}">
      <td class="small">${esc(formatTime(a.time))}</td>
      <td><span class="layer-badge l3">${esc(anomalyTypeLabel(a.type))}</span></td>
      <td>${governanceActionTag(a.action)}</td>
      <td>${a.risk_score ?? "—"}</td>
      <td><code>${esc(a.src_ip ?? workloadLabel(a.pod))}</code> → <code>${esc(a.access_addr ?? a.dst_ip ?? "—")}</code></td>
      <td class="small">${esc((a.message ?? "").slice(0, 80))}</td>
      <td><span class="net-action-btns">${ipActionButtons(a.dst_ip ?? a.src_ip, a.pod)} ${a.session_id ? btn({ label: "Graph", size: "sm", className: "btn-sg", attrs: { "data-sid": a.session_id } }) : btn({ label: "说明", size: "sm", className: "btn-explain", attrs: { "data-idx": String(idx) } })}</span></td>
    </tr>`,
      )
      .join("");
  }

  function bindSessionButtons(container: ParentNode): void {
    container.querySelectorAll(".btn-sg").forEach((b) => {
      b.addEventListener("click", () => {
        const sid = (b as HTMLElement).dataset.sid;
        if (sid) openSessionGraphModal(sid, toast);
      });
    });
    container.querySelectorAll(".btn-explain").forEach((b) => {
      b.addEventListener("click", () => {
        const idx = (b as HTMLElement).dataset.idx ?? "0";
        void apiGet<{ explain?: string; anomaly?: NerveAnomaly }>(`/api/nerve/observer/explain?index=${idx}`)
          .then((r) => toast(r.explain ?? r.anomaly?.message ?? "无说明", true))
          .catch((e) => toast(String(e), false));
      });
    });
  }

  function paintOverviewPanel(panel: ParentNode): void {
    const stats = networkStats as {
      total?: number;
      blocked?: number;
      trend?: { day: string; count: number }[];
      top_ips?: { ip: string; count: number }[];
      top_categories?: { category: string; count: number }[];
      top_pods?: { pod: string; count: number }[];
    };
    panel.innerHTML = `
      <div id="net-capability-kpis" class="kpi-row">加载 Agent 状态…</div>
      <div class="net-overview-grid" style="margin-top:12px">
        <div class="net-top-card">
          <h4>近 7 天 L3 事件趋势</h4>
          ${renderTrendBars(stats.trend ?? [])}
          <p class="muted small" style="margin-top:6px">归档 ${stats.total ?? 0} 条 · 拦截 ${stats.blocked ?? 0} 条 · 存储 ${esc(evBackend)}</p>
        </div>
        <div style="display:flex;flex-direction:column;gap:12px">
          <div class="net-top-card"><h4>Top 外联 IP</h4>
            ${(stats.top_ips ?? []).length
              ? (stats.top_ips ?? [])
                  .map(
                    (r) => `<div class="net-top-row"><span class="net-top-label"><code>${esc(r.ip)}</code></span>
                  <span class="net-top-count">${r.count}</span>${ipActionButtons(r.ip)}</div>`,
                  )
                  .join("")
              : `<p class="muted small">同步入库后显示</p>`}
          </div>
          ${renderTopList(
            "Top 异常类型",
            (stats.top_categories ?? []).map((r) => ({ label: categoryLabel(r.category), count: r.count })),
            "—",
          )}
          ${renderTopList(
            "Top 工作负载",
            (stats.top_pods ?? []).map((r) => ({ label: workloadLabel(r.pod), count: r.count })),
            "同步入库后显示",
          )}
        </div>
      </div>`;
    paintCapabilityKpis(panel);
    bindIpActions(panel, toast);
  }

  function topoMapOpts() {
    return { direction: topoDirection, topEdges: topoTopN, search: topoSearch, sourceLabel: topoSource };
  }

  function paintTopologyPanel(panel: ParentNode): void {
    const g = topoGraph ?? { nodes: [], edges: [] };
    const dirOpts = [
      { value: "all", label: "全部", selected: topoDirection === "all" },
      { value: "east_west", label: "东西向", selected: topoDirection === "east_west" },
      { value: "north_south", label: "南北向", selected: topoDirection === "north_south" },
    ];
    const topOpts = [
      { value: "20", label: "Top 20", selected: topoTopN === 20 },
      { value: "30", label: "Top 30", selected: topoTopN === 30 },
      { value: "50", label: "Top 50", selected: topoTopN === 50 },
    ];

    if (!topoMounted || !panel.querySelector("#net-topo-pf-root")) {
      topoMounted = true;
      panel.innerHTML = `
        <div class="kiali-graph-tab">
          <div class="kiali-graph-toolbar">
            ${tableToolbar(
              filterSelect({ id: "net-topo-dir", label: "Traffic", width: 110, options: dirOpts }),
              filterSelect({ id: "net-topo-topn", label: "Display", width: 100, options: topOpts }),
              searchInput({ id: "net-topo-search", placeholder: "Find / Hide", width: 180 }),
              `<span class="kiali-graph-src muted small">PatternFly Topology · ${esc(topoSource)}</span>`,
            )}
          </div>
          <div id="net-topo-graph">${serviceMapShellHtml()}</div>
        </div>`;
      bindTopologyControls(panel);
      void mountServiceMap(panel.querySelector("#net-topo-graph") as HTMLElement, g, topoMapOpts());
    } else {
      updateServiceMap(panel.querySelector("#net-topo-graph") as HTMLElement, g, topoMapOpts());
    }
  }

  function bindTopologyControls(panel: ParentNode): void {
    panel.querySelector("#net-topo-dir")?.addEventListener("change", (e) => {
      topoDirection = ((e.target as HTMLSelectElement).value as typeof topoDirection) || "all";
      if (topoGraph) paintTopologyPanel(panel);
    });
    panel.querySelector("#net-topo-topn")?.addEventListener("change", (e) => {
      topoTopN = Number((e.target as HTMLSelectElement).value) || 30;
      if (topoGraph) paintTopologyPanel(panel);
    });
    let searchTimer: ReturnType<typeof setTimeout> | null = null;
    panel.querySelector("#net-topo-search")?.addEventListener("input", (e) => {
      topoSearch = (e.target as HTMLInputElement).value;
      if (searchTimer) clearTimeout(searchTimer);
      searchTimer = setTimeout(() => {
        if (topoGraph) paintTopologyPanel(panel);
      }, 280);
    });
  }

  function paintLivePanel(): void {
    const panel = root.querySelector("#net-panel")!;
    if (activeTab !== "topology" && topoMounted) {
      topoMounted = false;
      destroyServiceMap(panel);
    }
    if (activeTab === "overview") {
      paintOverviewPanel(panel);
    } else if (activeTab === "topology") {
      paintTopologyPanel(panel);
    } else if (activeTab === "live-sessions") {
      const open = live.sessions?.open ?? [];
      const closed = live.sessions?.closed ?? [];
      const all = [...open, ...closed].slice(0, 100);
      panel.innerHTML = card({
        title: "连接会话",
        tip: `活跃 ${open.length} · 最近关闭 ${closed.length}`,
        body: `<div class="ant-table-wrapper">
          <table class="ant-table"><thead><tr>
            <th>状态</th><th>工作负载</th><th>目标 IP</th><th>地域</th><th>端口</th><th>时长</th><th>风险</th><th>策略命中</th><th>处置</th><th></th>
          </tr></thead><tbody>${sessionRows(all)}</tbody></table>
        </div>
        <p class="muted small" style="margin-top:8px">观测来源：cgroup sock eBPF · GeoIP 经 CIDR/MMDB · ASN/域名需扩展库</p>`,
      });
      bindSessionButtons(panel);
      bindIpActions(panel, toast);
      void hydrateSessionGeo(panel);
    } else if (activeTab === "live-anomalies") {
      const anomalies = live.anomalies ?? [];
      panel.innerHTML = card({
        title: "异常与分级治理",
        tip: `${anomalies.filter(isGovernanceAnomaly).length} 治理级 / ${anomalies.length} 总计`,
        body: `<label class="switch-wrap" style="margin-bottom:12px">
          <input type="checkbox" id="net-show-low" ${showLowPriority ? "checked" : ""} />
          <span>显示低优先级 orphan_connect</span>
        </label>
        <div class="ant-table-wrapper">
          <table class="ant-table"><thead><tr>
            <th>时间</th><th>类型</th><th>治理</th><th>风险</th><th>源 → 目标</th><th>消息</th><th></th>
          </tr></thead><tbody>${anomalyRows(anomalies)}</tbody></table>
        </div>
        <p class="muted small" style="margin-top:8px">L1 auto_deny / escalation_deny 写入 TC map · L2 escalation_alert 仅告警</p>`,
      });
      panel.querySelector("#net-show-low")?.addEventListener("change", (e) => {
        showLowPriority = (e.target as HTMLInputElement).checked;
        paintLivePanel();
      });
      bindSessionButtons(panel);
      bindIpActions(panel, toast);
    } else if (activeTab === "live-baseline") {
      const entries = live.baseline ?? [];
      panel.innerHTML = card({
        title: "工作负载行为基线",
        tip: `${entries.length} 个工作负载`,
        body: `<div class="ant-table-wrapper">
          <table class="ant-table"><thead><tr>
            <th>工作负载</th><th>阶段</th><th>已知目标数</th><th>违规次数</th><th>目标样本</th><th></th>
          </tr></thead>
          <tbody>${entries.length ? entries.map((b) => `<tr>
            <td>${esc(workloadLabel(b.pod))}</td>
            <td>${b.phase === "enforce" ? tag("enforce", "red") : tag("learning", "blue")}</td>
            <td>${(b.destinations ?? []).length}</td>
            <td>${b.violations ?? 0}</td>
            <td class="small muted">${esc((b.destinations ?? []).slice(0, 5).join(", ") || "—")}</td>
            <td>${btn({ label: "强制 enforce", size: "sm", className: "btn-net-enforce", attrs: { "data-pod": b.pod } })}</td>
          </tr>`).join("") : `<tr><td colspan="6" class="muted">暂无基线（出站 connect 后自动 learning）</td></tr>`}
          </tbody></table>
        </div>`,
      });
      panel.querySelectorAll(".btn-net-enforce").forEach((b) => {
        b.addEventListener("click", () => {
          const pod = (b as HTMLElement).dataset.pod!;
          void apiPost(`/api/nerve/baseline/enforce?pod=${encodeURIComponent(pod)}`)
            .then(() => toast(`已切换 ${workloadLabel(pod)} 为 enforce`, true))
            .catch((e) => toast(String(e), false));
        });
      });
    } else if (activeTab === "live-policy") {
      const hits = live.policy_hits ?? [];
      panel.innerHTML = card({
        title: "TC 策略命中（ringbuf）",
        tip: `${hits.length} 条最近命中`,
        body: `<div class="ant-table-wrapper">
          <table class="ant-table"><thead><tr>
            <th>时间</th><th>源 IP</th><th>地域</th><th>端口</th><th>动作</th><th>包长</th><th>累计</th><th>处置</th><th></th>
          </tr></thead>
          <tbody>${hits.length ? hits.map((h) => `<tr>
            <td class="small">${esc(formatTime(h.time))}</td>
            <td><code>${esc(h.src_ip)}</code></td>
            <td class="small geo-cell" data-ip="${esc(h.src_ip)}">${esc(geoCache.get(h.src_ip) ?? "…")}</td>
            <td>${h.dport ?? "—"}</td>
            <td>${tag(h.action ?? "deny", "red")}</td>
            <td>${h.pkt_len ?? "—"} B</td>
            <td>${h.total_hits ?? "—"}</td>
            <td>${ipActionButtons(h.src_ip)}</td>
            <td>${h.session_id ? btn({ label: "Graph", size: "sm", className: "btn-sg", attrs: { "data-sid": h.session_id } }) : ""}</td>
          </tr>`).join("") : `<tr><td colspan="9" class="muted">暂无（L1 deny 后入向包被 TC_ACT_SHOT 时出现）</td></tr>`}
          </tbody></table>
        </div>`,
      });
      bindSessionButtons(panel);
      bindIpActions(panel, toast);
      void hydrateSessionGeo(panel);
    } else {
      paintHistoryPanel(panel);
    }
  }

  function paintHistoryPanel(panel: HTMLElement): void {
    panel.innerHTML = `
      ${tableToolbar(
        filterSelect({ id: "net-filter-cat", label: "类型", width: 160, options: NETWORK_CATEGORY_OPTIONS }),
        btn({ id: "btn-net-hist-refresh", label: "刷新归档", variant: "primary" }),
      )}
      ${card({ id: "net-stats-card", title: "归档统计", bodyId: "net-stats", body: "—" })}
      ${card({ id: "net-table-card", title: "PostgreSQL 事件（poll 入库）", bodyId: "net-table", body: '<div class="page-loading">加载中…</div>' })}`;

    function paintStats(): void {
      const el = panel.querySelector("#net-stats")!;
      const orphan = cachedEvents.filter((e) => e.category === "orphan_connect").length;
      const high = cachedEvents.filter((e) => e.severity === "high" || e.severity === "critical").length;
      const scan = cachedEvents.filter((e) => e.category === "port_scan").length;
      el.innerHTML = `<div class="kpi-row" style="margin:0">
        <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">样本事件</div><div class="kpi-value">${cachedEvents.length}</div></div></div>
        <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">孤儿连接</div><div class="kpi-value kpi-warn">${orphan}</div></div></div>
        <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">高危/组合</div><div class="kpi-value kpi-danger">${high}</div></div></div>
        <div class="kpi-card"><div class="kpi-body"><div class="kpi-label">端口扫描</div><div class="kpi-value">${scan}</div></div></div>
      </div>`;
    }

    function paintTable(): void {
      const table = panel.querySelector("#net-table")!;
      if (!cachedEvents.length) {
        table.innerHTML = `<div class="ant-empty">暂无归档事件（backend: ${esc(evBackend)}）<br><span class="muted small">点击「同步入库」将 Agent 异常/审计写入 PostgreSQL</span></div>`;
        return;
      }
      const section = renderPaginatedSection("net-list", cachedEvents, evPage, (pageEvents) => {
        const rows = pageEvents
          .map(
            (e) => `<tr>
          <td><code class="small">${esc(e.ts.slice(0, 19).replace("T", " "))}</code></td>
          <td>${actionTag(e.action)}</td>
          <td><span class="layer-badge l3">${esc(categoryLabel(e.category))}</span></td>
          <td><code>${esc(e.src_ip ?? e.dst_ip ?? "—")}</code>${e.pod ? `<br><span class="muted small">${esc(e.pod)}</span>` : ""}</td>
          <td class="small">${esc(String(e.message ?? e.uri ?? "—").slice(0, 100))}</td>
        </tr>`,
          )
          .join("");
        return `<div class="muted table-summary">共 ${evTotal} 条 · 存储: ${esc(evBackend)}</div>
          <div class="ant-table-wrapper"><table class="ant-table"><thead><tr>
            <th>时间</th><th>动作</th><th>类型</th><th>源 / Pod</th><th>消息</th>
          </tr></thead><tbody>${rows}</tbody></table></div>`;
      });
      evPage.page = section.page;
      table.innerHTML = section.html;
      bindPaginationBar(table, "net-list", evPage, cachedEvents.length, (next) => {
        evPage.page = next.page;
        evPage.pageSize = next.pageSize;
        paintTable();
      });
    }

    async function loadHistory(): Promise<void> {
      const cat = (panel.querySelector("#net-filter-cat") as HTMLSelectElement | null)?.value ?? "";
      const q = new URLSearchParams({ limit: "100", layer: "l3" });
      if (cat) q.set("category", cat);
      const data = await apiGet<{ total: number; events: WafEvent[]; backend: string }>(`/api/events?${q}`);
      cachedEvents = data.events;
      evBackend = data.backend;
      evTotal = data.total;
      evPage.page = 1;
      paintStats();
      paintTable();
    }

    panel.querySelector("#btn-net-hist-refresh")?.addEventListener("click", () => void loadHistory().catch((e) => toast(String(e), false)));
    panel.querySelector("#net-filter-cat")?.addEventListener("change", () => void loadHistory().catch((e) => toast(String(e), false)));
    void loadHistory().catch((e) => toast(String(e), false));
  }

  async function refreshLive(): Promise<void> {
    if (paused) return;
    try {
      const [state, nerve, sessions, statsRes, topoStatus] = await Promise.all([
        apiGet<NerveLiveState>("/api/nerve/observer/live"),
        apiGet<{ policy?: { enabled?: boolean; ifaces?: string[] } }>("/api/nerve/status"),
        apiGet<{ open?: FlowSession[]; closed?: FlowSession[] }>("/api/nerve/observer/sessions?limit=50"),
        apiGet<{ stats: Record<string, unknown>; backend: string }>("/api/network/stats?days=7").catch(() => ({
          stats: {},
          backend: "—",
        })),
        apiGet<{ enabled: boolean }>("/api/topology/status").catch(() => ({ enabled: false })),
      ]);
      live = state;
      live.sessions = { open: sessions.open ?? state.sessions?.open ?? [], closed: sessions.closed ?? [] };
      live.status = { ...(live.status ?? {}), policy: nerve.policy };
      networkStats = statsRes.stats ?? {};
      evBackend = statsRes.backend ?? evBackend;

      const allSessions = [...(live.sessions.open ?? []), ...(live.sessions.closed ?? [])];
      let apiGraph: TopologyGraph | null = null;
      if (topoStatus.enabled) {
        try {
          apiGraph = await apiGet<TopologyGraph>("/api/topology/graph?minutes=60&direction=all");
        } catch {
          apiGraph = null;
        }
      }
      const fromSessions = sessionsToGraph(allSessions);
      topoGraph = effectiveTopology(apiGraph, allSessions);
      topoSource =
        apiGraph?.nodes?.length ? "PostgreSQL flow-relay" : fromSessions.nodes.length ? "实时会话回退" : "无数据";

      setAgentBadge(true, `Agent 在线 · ${new Date().toLocaleTimeString()}`);
      if (activeTab !== "history") paintLivePanel();
    } catch (e) {
      setAgentBadge(false, "Agent 不可达");
      live = { error: String(e) };
    }
  }

  root.querySelectorAll("[data-tab]").forEach((tab) => {
    tab.addEventListener("click", () => {
      activeTab = (tab as HTMLElement).dataset.tab ?? "overview";
      root.querySelectorAll("[data-tab]").forEach((t) => t.classList.toggle("active", t === tab));
      paintLivePanel();
    });
  });

  root.querySelector("#btn-net-refresh")?.addEventListener("click", () => void refreshLive());
  root.querySelector("#btn-net-pause")?.addEventListener("click", (e) => {
    const btnEl = e.currentTarget as HTMLButtonElement;
    paused = !paused;
    btnEl.textContent = paused ? "继续刷新" : "暂停刷新";
  });
  root.querySelector("#btn-net-poll-db")?.addEventListener("click", () => {
    void runMutatingAction({
      action: "save",
      subject: "网络观测归档",
      confirmMessage: "将 Agent 异常、审计、policy_hit 同步写入 PostgreSQL？",
      successMessage: "同步完成",
      run: async () => {
        const r = await apiPost<{ inserted: number }>("/api/events/poll");
        toast(`已入库 ${r.inserted ?? 0} 条`, true);
        if (activeTab === "history") paintLivePanel();
      },
    });
  });

  await refreshLive();
  paintLivePanel();

  pollTimer = setInterval(() => {
    if (!document.getElementById("net-live-root")) {
      stopPolling();
      return;
    }
    void refreshLive();
  }, 2500);
}
