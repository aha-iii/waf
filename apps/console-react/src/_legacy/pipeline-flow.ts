/**
 * 检测流水线可视化编排（LogicFlow）
 */
import LogicFlow, {
  CircleNode,
  CircleNodeModel,
  HtmlNode,
  HtmlNodeModel,
  type EdgeData,
  type GraphData,
  type NodeData,
} from "@logicflow/core";
import { Control, DndPanel, MiniMap, SelectionSelect } from "@logicflow/extension";
import "@logicflow/core/lib/style/index.css";
import "@logicflow/extension/lib/style/index.css";
import {
  PIPELINE_STAGES,
  defaultPipelineEnabled,
  normalizePipelineEnabled,
  type PipelineEnabledMap,
} from "./pipeline-stages.js";
import { sanitizeProductText } from "./brand-sanitize.js";

export type PipelineNodeLayout = { x: number; y: number };

export type PipelineGraphData = {
  nodes: NodeData[];
  edges: EdgeData[];
};

export type PipelineEditorState = {
  mode: "list" | "flow";
  layout: Record<string, PipelineNodeLayout>;
  graph?: PipelineGraphData | null;
};

const STAGE_ICON: Record<string, string> = {
  semantic: "语",
  expert: "专",
  core: "核",
  custom: "自",
  plugin: "插",
  crs: "专",
};

const STAGE_COLOR: Record<string, string> = {
  semantic: "#597ef7",
  expert: "#fa8c16",
  core: "#1890ff",
  custom: "#9254de",
  plugin: "#13c2c2",
  crs: "#fa8c16",
};

const STAGE_ACCENT_CLASS: Record<string, string> = {
  semantic: "lf-card-semantic",
  expert: "lf-card-crs",
  core: "lf-card-core",
  custom: "lf-card-custom",
  plugin: "lf-card-plugin",
  crs: "lf-card-crs",
};

const DEFAULT_LAYOUT: Record<string, PipelineNodeLayout> = {
  semantic: { x: 0, y: 0 },
  expert: { x: 0, y: 0 },
  core: { x: 0, y: 0 },
  custom: { x: 0, y: 0 },
  plugin: { x: 0, y: 0 },
  crs: { x: 0, y: 0 },
};

const CANVAS_X = 420;
const START_Y = 70;
const GAP_Y = 120;

const lfStore = new WeakMap<HTMLElement, LogicFlow>();

LogicFlow.use(DndPanel);
LogicFlow.use(Control);
LogicFlow.use(MiniMap);
LogicFlow.use(SelectionSelect);

class PipelineStartModel extends CircleNodeModel {
  initNodeData(data: LogicFlow.NodeConfig): void {
    super.initNodeData(data);
    this.r = 32;
    this.text.value = "开始";
  }
  getNodeStyle(): LogicFlow.PropertiesType {
    const style = super.getNodeStyle();
    style.stroke = "#52c41a";
    style.fill = "#f6ffed";
    style.strokeWidth = 2;
    return style;
  }
}

class PipelineEndModel extends CircleNodeModel {
  initNodeData(data: LogicFlow.NodeConfig): void {
    super.initNodeData(data);
    this.r = 32;
    this.text.value = "结束";
  }
  getNodeStyle(): LogicFlow.PropertiesType {
    const style = super.getNodeStyle();
    style.stroke = "#1890ff";
    style.fill = "#e6f7ff";
    style.strokeWidth = 2;
    return style;
  }
}

class PipelineStageModel extends HtmlNodeModel {
  initNodeData(data: LogicFlow.NodeConfig): void {
    super.initNodeData(data);
    this.width = 248;
    this.height = 78;
    this.text.value = "";
    this.text.editable = false;
  }
  getDefaultAnchor() {
    const { x, y, height } = this;
    return [
      { x, y: y - height / 2, id: `${this.id}_top` },
      { x, y: y + height / 2, id: `${this.id}_bottom` },
    ];
  }
}

class PipelineStageView extends HtmlNode {
  setHtml(rootEl: SVGForeignObjectElement): void {
    const { properties, id } = this.props.model;
    const stageId = String(properties.stageId ?? "");
    const meta = stageMeta(stageId);
    const desc = sanitizeProductText(meta.desc);
    const step = properties.step ?? "";
    const enabled = properties.enabled !== false;
    const label = sanitizeProductText(meta.label);
    const accent = STAGE_ACCENT_CLASS[stageId] ?? STAGE_ACCENT_CLASS[meta.id] ?? "";
    const color = STAGE_COLOR[stageId] ?? STAGE_COLOR[meta.id] ?? "#1890ff";
    const badge = STAGE_ICON[stageId] ?? STAGE_ICON[meta.id] ?? "●";

    while (rootEl.firstChild) rootEl.removeChild(rootEl.firstChild);
    const wrap = document.createElementNS("http://www.w3.org/1999/xhtml", "div");
    wrap.setAttribute("xmlns", "http://www.w3.org/1999/xhtml");
    wrap.className = `lf-pipeline-card ${accent}${enabled ? "" : " is-off"}`;
    wrap.innerHTML = `
      <div class="lf-pipeline-card-icon" style="background:${enabled ? color : "#d9d9d9"}">${badge}</div>
      <div class="lf-pipeline-card-body">
        <div class="lf-pipeline-card-head"><strong>${label}</strong><span class="lf-pipeline-step">${step}</span></div>
        <div class="lf-pipeline-card-desc">${desc}</div>
        <code class="lf-pipeline-card-id">${meta.id}</code>
      </div>
      <label class="lf-pipeline-enable-wrap" style="pointer-events:auto">
        <input type="checkbox" class="lf-pipeline-enable" data-node-id="${id}" ${enabled ? "checked" : ""} />
        <span>启用</span>
      </label>`;
    rootEl.appendChild(wrap);
  }
}

function stageMeta(id: string): { id: string; label: string; desc: string } {
  const mapped = id === "crs" ? "expert" : id;
  const hit = PIPELINE_STAGES.find((s) => s.id === mapped);
  if (hit) return { id: hit.id, label: hit.label, desc: hit.desc };
  if (mapped === "semantic") {
    return { id: "semantic", label: "语义增强", desc: "解码链与意图检测，挂在核心防护段" };
  }
  if (mapped === "plugin") {
    return { id: "plugin", label: "自定义插件", desc: "L7 自定义插件" };
  }
  return { id: mapped, label: mapped, desc: "" };
}

function svgPaletteIcon(color: string, text: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="36" height="36"><rect width="36" height="36" rx="8" fill="${color}"/><text x="18" y="23" text-anchor="middle" fill="#fff" font-size="11" font-family="sans-serif" font-weight="700">${text}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export function defaultPipelineEditor(): PipelineEditorState {
  return { mode: "list", layout: { ...DEFAULT_LAYOUT }, graph: null };
}

/** 按列表顺序重建 LogicFlow 图，保证 graph 与 pipeline_order 一致 */
export function rebuildPipelineGraph(
  order: string[],
  enabled: PipelineEnabledMap,
): PipelineGraphData {
  return buildDefaultGraph(order, enabled);
}

export function mergePipelineEditor(raw?: Partial<PipelineEditorState> | null): PipelineEditorState {
  const d = defaultPipelineEditor();
  if (!raw) return d;
  return {
    mode: raw.mode === "list" || raw.mode === "flow" ? raw.mode : d.mode,
    layout: { ...d.layout, ...(raw.layout ?? {}) },
    graph: raw.graph ?? d.graph,
  };
}

function buildDefaultGraph(order: string[], enabled: PipelineEnabledMap): PipelineGraphData {
  const chain = order.length ? order : PIPELINE_STAGES.map((s) => s.id);
  const en = normalizePipelineEnabled(enabled);
  const nodes: NodeData[] = [
    {
      id: "lf-start",
      type: "pipeline-start",
      x: CANVAS_X,
      y: START_Y,
      text: "开始",
      properties: { kind: "start" },
    },
  ];
  const edges: EdgeData[] = [];
  let prev = "lf-start";
  let y = START_Y + GAP_Y;

  let exec = 0;
  chain.forEach((stageId) => {
    const meta = stageMeta(stageId);
    const nid = `stage-${stageId}`;
    const on = en[stageId] !== false;
    if (on) exec += 1;
    nodes.push({
      id: nid,
      type: "pipeline-stage",
      x: CANVAS_X,
      y,
      text: "",
      properties: { stageId, step: on ? exec : "—", desc: meta.desc, enabled: on },
    });
    edges.push({
      id: `e-${prev}-${nid}`,
      type: "polyline",
      sourceNodeId: prev,
      targetNodeId: nid,
    });
    prev = nid;
    y += GAP_Y;
  });

  nodes.push({
    id: "lf-end",
    type: "pipeline-end",
    x: CANVAS_X,
    y,
    text: "结束",
    properties: { kind: "end" },
  });
  edges.push({
    id: `e-${prev}-lf-end`,
    type: "polyline",
    sourceNodeId: prev,
    targetNodeId: "lf-end",
  });

  return { nodes, edges };
}

function applyEnabledToGraph(graph: PipelineGraphData, enabled: PipelineEnabledMap): PipelineGraphData {
  const en = normalizePipelineEnabled(enabled);
  return {
    nodes: graph.nodes.map((n) => {
      const sid = n.properties?.stageId;
      if (typeof sid !== "string") return n;
      const on = en[sid] !== false;
      return { ...n, properties: { ...n.properties, enabled: on } };
    }),
    edges: graph.edges,
  };
}

function resolveGraph(
  order: string[],
  enabled: PipelineEnabledMap,
  saved?: PipelineGraphData | null,
): PipelineGraphData {
  if (saved?.nodes?.length) {
    const hasStart = saved.nodes.some((n) => n.id === "lf-start" || n.type === "pipeline-start");
    const hasEnd = saved.nodes.some((n) => n.id === "lf-end" || n.type === "pipeline-end");
    if (hasStart && hasEnd) return applyEnabledToGraph(saved, enabled);
  }
  return buildDefaultGraph(order, enabled);
}

/** 画布链路上的顺序（不自动补全已删除的段） */
export function extractChainOrderFromGraph(data: GraphData | PipelineGraphData): string[] {
  const nodes = data.nodes ?? [];
  const edges = data.edges ?? [];
  const start =
    nodes.find((n) => n.id === "lf-start") ?? nodes.find((n) => n.type === "pipeline-start");
  if (!start) return [];
  let cur = start.id;
  const order: string[] = [];
  const seen = new Set<string>();

  for (let i = 0; i < 16; i++) {
    const edge = edges.find((e) => e.sourceNodeId === cur);
    if (!edge) break;
    const target = nodes.find((n) => n.id === edge.targetNodeId);
    if (!target) break;
    if (target.id === "lf-end" || target.type === "pipeline-end") break;
    const sid = target.properties?.stageId;
    if (typeof sid === "string" && !seen.has(sid)) {
      order.push(sid);
      seen.add(sid);
    }
    cur = target.id;
  }
  return order;
}

/** 列表模式用：画布顺序 + 未出现在画布上的段排到末尾 */
export function mergePipelineOrderForList(chainOrder: string[]): string[] {
  const seen = new Set(chainOrder);
  const tail = PIPELINE_STAGES.map((s) => s.id).filter((id) => !seen.has(id));
  return chainOrder.length ? [...chainOrder, ...tail] : PIPELINE_STAGES.map((s) => s.id);
}

export function extractOrderFromGraph(data: GraphData | PipelineGraphData): string[] {
  const chain = extractChainOrderFromGraph(data);
  return mergePipelineOrderForList(chain);
}

export function extractEnabledFromGraph(data: GraphData | PipelineGraphData): PipelineEnabledMap {
  const map = defaultPipelineEnabled();
  const present = new Set<string>();
  for (const node of data.nodes ?? []) {
    const sid = node.properties?.stageId;
    if (typeof sid === "string") {
      present.add(sid);
      map[sid] = node.properties?.enabled !== false;
    }
  }
  for (const st of PIPELINE_STAGES) {
    if (!present.has(st.id)) map[st.id] = false;
  }
  return normalizePipelineEnabled(map);
}

function extractLayoutFromGraph(data: GraphData | PipelineGraphData): Record<string, PipelineNodeLayout> {
  const layout: Record<string, PipelineNodeLayout> = { ...DEFAULT_LAYOUT };
  for (const node of data.nodes ?? []) {
    const sid = node.properties?.stageId;
    if (typeof sid !== "string") continue;
    layout[sid] = {
      x: Math.round((node.x ?? CANVAS_X) - CANVAS_X),
      y: 0,
    };
  }
  return layout;
}

function registerPipelineNodes(lf: LogicFlow): void {
  lf.register({ type: "pipeline-start", view: CircleNode, model: PipelineStartModel });
  lf.register({ type: "pipeline-end", view: CircleNode, model: PipelineEndModel });
  lf.register({ type: "pipeline-stage", view: PipelineStageView, model: PipelineStageModel });
}

function setupDndPanel(lf: LogicFlow): void {
  const ext = lf.extension.dndPanel as DndPanel | undefined;
  if (!ext?.setPatternItems) return;

  const stages = PIPELINE_STAGES.map((st) => ({
    type: "pipeline-stage",
    label: st.label,
    icon: svgPaletteIcon(STAGE_COLOR[st.id] ?? "#1890ff", STAGE_ICON[st.id] ?? "?"),
    properties: { stageId: st.id, desc: st.desc, enabled: true },
  }));

  ext.setPatternItems([
    {
      label: "开始",
      type: "pipeline-start",
      icon: svgPaletteIcon("#52c41a", "▶"),
      properties: { kind: "start" },
    },
    ...stages,
    {
      label: "结束",
      type: "pipeline-end",
      icon: svgPaletteIcon("#1890ff", "⏹"),
      properties: { kind: "end" },
    },
  ]);
}

function refreshStageSteps(lf: LogicFlow): void {
  const data = lf.getGraphData();
  const order = extractChainOrderFromGraph(data);
  let exec = 0;
  order.forEach((stageId) => {
    const node = data.nodes.find((n) => n.properties?.stageId === stageId);
    if (!node) return;
    const enabled = node.properties?.enabled !== false;
    const step = enabled ? ++exec : "—";
    lf.setProperties(node.id, { ...node.properties, step });
  });
}

export function destroyPipelineFlowEditor(container: HTMLElement): void {
  const ext = container as HTMLElement & { _lfFsCleanup?: () => void };
  ext._lfFsCleanup?.();
  ext._lfFsCleanup = undefined;
  if (document.fullscreenElement?.id === "pipeline-lf-shell") {
    void document.exitFullscreen().catch(() => null);
  }
  const lf = lfStore.get(container);
  if (lf) {
    lf.destroy();
    lfStore.delete(container);
  }
}

export function renderPipelineFlowEditor(
  order: string[],
  editor: PipelineEditorState,
  _esc: (s: string) => string,
): string {
  return `<div class="pipeline-lf-shell" id="pipeline-lf-shell">
    <div class="pipeline-lf-toolbar">
      <span class="pipeline-lf-brand">LogicFlow 编排</span>
      <button type="button" class="ant-btn ant-btn-sm" id="flow-fullscreen">全屏</button>
      <button type="button" class="ant-btn ant-btn-sm" id="flow-auto-layout">自动布局</button>
      <button type="button" class="ant-btn ant-btn-sm" id="flow-fit-view">适应画布</button>
    </div>
    <div class="pipeline-lf-host" id="app-pipeline-lf-host"></div>
  </div>`;
}

export function bindPipelineFlowEditor(
  container: HTMLElement,
  order: string[],
  editor: PipelineEditorState,
  enabled: PipelineEnabledMap,
  onChange?: () => void,
): void {
  destroyPipelineFlowEditor(container);

  const host = container.querySelector("#app-pipeline-lf-host") as HTMLElement | null;
  if (!host) return;

  const graph = resolveGraph(order, enabled, editor.graph);

  const lf = new LogicFlow({
    container: host,
    grid: {
      size: 12,
      visible: true,
      type: "dot",
      config: { color: "#d0d5dd", thickness: 1 },
    },
    keyboard: { enabled: true },
    edgeType: "polyline",
    adjustEdge: true,
    adjustNodePosition: true,
    hoverOutline: true,
    nodeSelectedOutline: true,
    plugins: [DndPanel, Control, MiniMap, SelectionSelect],
    pluginsOptions: {
      miniMap: {
        width: 140,
        height: 90,
        rightPosition: 12,
        bottomPosition: 12,
      },
    },
  });

  registerPipelineNodes(lf);
  lf.setTheme({
    polyline: { stroke: "#1890ff", strokeWidth: 2 },
    arrow: { offset: 6, verticalLength: 4 },
    anchor: { stroke: "#1890ff", fill: "#fff", r: 4, hover: { fill: "#1890ff" } },
  });

  lf.render(graph);
  setupDndPanel(lf);
  refreshStageSteps(lf);

  const notify = (): void => {
    refreshStageSteps(lf);
    onChange?.();
  };

  lf.on("edge:add", notify);
  lf.on("edge:delete", notify);
  lf.on("node:drop", notify);
  lf.on("node:delete", notify);

  lf.on("node:add", ({ data }) => {
    if (data.type === "pipeline-start" || data.type === "pipeline-end") {
      const same = lf.getGraphData().nodes.filter((n) => n.type === data.type);
      if (same.length > 1) {
        // 已有开始/结束时，丢弃新拖入的重复节点
        const keep =
          same.find((n) => n.id === (data.type === "pipeline-start" ? "lf-start" : "lf-end")) ??
          same.find((n) => n.id !== data.id) ??
          same[0];
        for (const n of same) {
          if (n.id !== keep.id) lf.deleteNode(n.id);
        }
        return;
      }
      notify();
      return;
    }
    if (data.type !== "pipeline-stage") return;
    const sid = data.properties?.stageId;
    if (typeof sid !== "string") return;
    const dupes = lf.getGraphData().nodes.filter((n) => n.properties?.stageId === sid);
    if (dupes.length > 1) {
      lf.deleteNode(data.id);
      return;
    }
    if (!data.properties?.desc) {
      const meta = stageMeta(sid);
      lf.setProperties(data.id, {
        ...data.properties,
        stageId: sid,
        desc: meta.desc,
        enabled: data.properties?.enabled !== false,
      });
    }
    notify();
  });

  host.addEventListener("change", (e) => {
    const t = e.target as HTMLInputElement;
    if (!t.classList.contains("lf-pipeline-enable")) return;
    const nodeId = t.dataset.nodeId;
    if (!nodeId) return;
    const model = lf.getNodeModelById(nodeId);
    if (!model) return;
    lf.setProperties(nodeId, { ...model.properties, enabled: t.checked });
    notify();
  });

  container.querySelector("#flow-auto-layout")?.addEventListener("click", () => {
    const currentOrder = extractChainOrderFromGraph(lf.getGraphData());
    const currentEnabled = extractEnabledFromGraph(lf.getGraphData());
    lf.render(buildDefaultGraph(currentOrder, currentEnabled));
    refreshStageSteps(lf);
    lf.fitView(40, 40);
    onChange?.();
  });

  container.querySelector("#flow-fit-view")?.addEventListener("click", () => {
    lf.fitView(40, 40);
  });

  const shell = container.querySelector("#pipeline-lf-shell") as HTMLElement | null;
  const fsBtn = container.querySelector("#flow-fullscreen") as HTMLButtonElement | null;
  const onFsChange = (): void => {
    const full = document.fullscreenElement === shell;
    if (fsBtn) fsBtn.textContent = full ? "退出全屏" : "全屏";
    shell?.classList.toggle("is-fullscreen", full);
    requestAnimationFrame(() => {
      lf.resize();
      lf.fitView(40, 40);
    });
  };
  fsBtn?.addEventListener("click", () => {
    if (!shell) return;
    if (document.fullscreenElement === shell) {
      void document.exitFullscreen();
    } else {
      void shell.requestFullscreen();
    }
  });
  document.addEventListener("fullscreenchange", onFsChange);
  (container as HTMLElement & { _lfFsCleanup?: () => void })._lfFsCleanup = () => {
    document.removeEventListener("fullscreenchange", onFsChange);
  };

  requestAnimationFrame(() => lf.fitView(40, 40));
  lfStore.set(container, lf);
}

/** 去掉 LogicFlow 运行时字段（如 pointsList），减小写入体积。 */
export function compactPipelineGraph(graph: PipelineGraphData | null | undefined): PipelineGraphData | null {
  if (!graph?.nodes?.length) return graph ?? null;
  const stripNode = ({ id, type, x, y, text, properties }: NodeData): NodeData => ({
    id,
    type,
    x,
    y,
    text,
    properties,
  });
  const stripEdge = ({ id, type, sourceNodeId, targetNodeId, properties }: EdgeData): EdgeData => ({
    id,
    type,
    sourceNodeId,
    targetNodeId,
    properties,
  });
  return {
    nodes: graph.nodes.map(stripNode),
    edges: (graph.edges ?? []).map(stripEdge),
  };
}

export function readPipelineFlowState(container: HTMLElement | null): {
  order: string[];
  layout: Record<string, PipelineNodeLayout>;
  enabled: PipelineEnabledMap;
  graph: PipelineGraphData | null;
} {
  if (!container) return { order: [], layout: {}, enabled: defaultPipelineEnabled(), graph: null };
  const lf = lfStore.get(container);
  if (!lf) return { order: [], layout: {}, enabled: defaultPipelineEnabled(), graph: null };

  const graph = lf.getGraphData() as PipelineGraphData;
  return {
    order: extractOrderFromGraph(graph),
    layout: extractLayoutFromGraph(graph),
    enabled: extractEnabledFromGraph(graph),
    graph,
  };
}

export function readPipelineFlowOrder(container: HTMLElement | null): string[] {
  return readPipelineFlowState(container).order;
}

export function readPipelineFlowLayout(container: HTMLElement | null): Record<string, PipelineNodeLayout> {
  return readPipelineFlowState(container).layout;
}

export function bindPipelineModeSwitch(
  container: HTMLElement,
  onSwitch: (oldMode: "list" | "flow", newMode: "list" | "flow") => void,
): void {
  container.querySelectorAll<HTMLButtonElement>(".pipeline-mode-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const newMode = btn.dataset.mode === "list" ? "list" : "flow";
      const oldMode = getPipelineEditorMode(container);
      if (oldMode === newMode) return;
      onSwitch(oldMode, newMode);
      const hidden = container.querySelector("#app-pipeline-editor-mode") as HTMLInputElement | null;
      if (hidden) hidden.value = newMode;
      container.querySelectorAll(".pipeline-mode-btn").forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      container.querySelector("#pipeline-panel-list")?.classList.toggle("hidden", newMode !== "list");
      container.querySelector("#pipeline-panel-flow")?.classList.toggle("hidden", newMode !== "flow");
    });
  });
}

export function getPipelineEditorMode(container: HTMLElement): "list" | "flow" {
  const hidden = container.querySelector("#app-pipeline-editor-mode") as HTMLInputElement | null;
  return hidden?.value === "list" ? "list" : "flow";
}
