/**
 * PatternFly Topology · Kiali 同款 Graph 组件
 */
import * as React from "react";
import {
  Divider,
  Dropdown,
  DropdownItem,
  DropdownList,
  MenuToggle,
  type MenuToggleElement,
  Title,
  ToolbarItem,
} from "@patternfly/react-core";
import {
  BlueprintIcon,
  CloudSecurityIcon,
  CubesIcon,
  ServiceIcon,
} from "@patternfly/react-icons";
import {
  action,
  BreadthFirstLayout,
  ColaGroupsLayout,
  ComponentFactory,
  ConcentricLayout,
  createTopologyControlButtons,
  defaultControlButtonsOptions,
  DagreLayout,
  DefaultEdge,
  DefaultGroup,
  DefaultNode,
  EdgeAnimationSpeed,
  EdgeStyle,
  EdgeTerminalType,
  GRAPH_LAYOUT_END_EVENT,
  Graph,
  GraphComponent,
  Layout,
  LayoutFactory,
  Model,
  ModelKind,
  Node,
  NodeModel,
  NodeShape,
  NodeStatus,
  observer,
  SELECTION_EVENT,
  TopologyControlBar,
  TopologyView,
  Visualization,
  VisualizationProvider,
  VisualizationSurface,
  withPanZoom,
  withSelection,
  type WithSelectionProps,
} from "@patternfly/react-topology";
import "@patternfly/react-core/dist/styles/base.css";
import "@patternfly/react-topology/dist/esm/css/topology-components.css";
import "@patternfly/react-topology/dist/esm/css/topology-controlbar.css";
import "@patternfly/react-topology/dist/esm/css/topology-view.css";
import "./network-topology-pf.css";
import {
  pruneServiceMapGraph,
  shortTopologyLabel,
  topologyNodeKind,
  type TopologyEdge,
  type TopologyGraph,
  type TopologyNode,
} from "./network-topology.js";

export type PfTopologyProps = {
  graph: TopologyGraph;
  direction?: "all" | "east_west" | "north_south";
  topEdges?: number;
  search?: string;
  sourceLabel?: string;
};

function filterBySearch(graph: TopologyGraph, search: string): TopologyGraph {
  const q = search.trim().toLowerCase();
  if (!q) return graph;
  const nodes = graph.nodes.filter(
    (n) =>
      n.label.toLowerCase().includes(q) ||
      n.id.toLowerCase().includes(q) ||
      (n.namespace ?? "").toLowerCase().includes(q),
  );
  const ids = new Set(nodes.map((n) => n.id));
  return {
    nodes,
    edges: graph.edges.filter((e) => ids.has(e.source) && ids.has(e.target)),
    stats: graph.stats,
  };
}

function trafficOf(edges: TopologyEdge[]): Map<string, { in: number; out: number }> {
  const m = new Map<string, { in: number; out: number }>();
  for (const e of edges) {
    const s = m.get(e.source) ?? { in: 0, out: 0 };
    const t = m.get(e.target) ?? { in: 0, out: 0 };
    s.out += e.count;
    t.in += e.count;
    m.set(e.source, s);
    m.set(e.target, t);
  }
  return m;
}

function nodeShape(kind: ReturnType<typeof topologyNodeKind>): NodeShape {
  switch (kind) {
    case "service":
      return NodeShape.rhombus;
    case "workload":
      return NodeShape.rect;
    case "external":
      return NodeShape.ellipse;
    case "infra":
      return NodeShape.hexagon;
    default:
      return NodeShape.stadium;
  }
}

function nodeIcon(kind: ReturnType<typeof topologyNodeKind>): React.ComponentType<{ style?: React.CSSProperties }> {
  switch (kind) {
    case "service":
      return ServiceIcon;
    case "external":
      return CloudSecurityIcon;
    case "workload":
      return CubesIcon;
    default:
      return BlueprintIcon;
  }
}

type LayoutName = "Dagre" | "ColaGroups" | "Concentric" | "BreadthFirst";

const LAYOUT_LABELS: Record<LayoutName, string> = {
  Dagre: "Dagre · 左右分层（推荐）",
  ColaGroups: "ColaGroups · 分组力导向",
  Concentric: "Concentric · kiali-concentric",
  BreadthFirst: "BreadthFirst · 仅适合小图",
};

function isServiceSide(kind: ReturnType<typeof topologyNodeKind>): boolean {
  return kind === "service" || kind === "external";
}

function buildModel(graph: TopologyGraph, layout: LayoutName): Model {
  const traffic = trafficOf(graph.edges);
  const hideEdgeLabels = graph.edges.length > 18;
  const workloadIds: string[] = [];
  const serviceIds: string[] = [];
  const leafNodes: NodeModel[] = [];

  for (const n of graph.nodes) {
    const kind = topologyNodeKind(n);
    const t = traffic.get(n.id) ?? { in: 0, out: 0 };
    const isService = isServiceSide(kind);
    const id = n.id;
    leafNodes.push({
      id,
      type: "node",
      label: shortTopologyLabel(n),
      width: isService ? 64 : 76,
      height: isService ? 56 : 68,
      shape: nodeShape(kind),
      status: NodeStatus.success,
      parent: isService ? "group:services" : "group:workloads",
      data: { kind, raw: n, trafficIn: t.in, trafficOut: t.out },
    });
    if (isService) serviceIds.push(id);
    else workloadIds.push(id);
  }

  const groups: NodeModel[] = [
    {
      id: "group:workloads",
      type: "group",
      group: true,
      label: `Workloads (${workloadIds.length})`,
      children: workloadIds,
      style: { padding: 56 },
    },
    {
      id: "group:services",
      type: "group",
      group: true,
      label: `Services (${serviceIds.length})`,
      children: serviceIds,
      style: { padding: 56 },
    },
  ];

  const edges = graph.edges.map((e, i) => ({
    id: `edge-${i}-${e.source}-${e.target}`,
    type: "edge",
    source: e.source,
    target: e.target,
    label: !hideEdgeLabels && e.count >= 5 ? String(e.count) : undefined,
    edgeStyle: EdgeStyle.default,
    animationSpeed:
      graph.edges.length > 24
        ? EdgeAnimationSpeed.none
        : e.count >= 30
          ? EdgeAnimationSpeed.medium
          : EdgeAnimationSpeed.slow,
    endTerminalType: EdgeTerminalType.directional,
  }));

  return {
    nodes: [...groups, ...leafNodes],
    edges,
    graph: { id: "nerve-graph", type: "graph", layout },
  };
}

const LAYOUT_OPTS = {
  nodeDistance: 85,
  groupDistance: 140,
  linkDistance: 130,
  collideDistance: 75,
  layoutOnDrag: false,
} as const;

const layoutFactory: LayoutFactory = (type: string, graph: Graph): Layout | undefined => {
  switch (type) {
    case "Dagre":
      return new DagreLayout(graph, {
        ...LAYOUT_OPTS,
        rankdir: "LR",
        nodesep: 55,
        ranksep: 110,
        marginx: 48,
        marginy: 48,
      });
    case "ColaGroups":
      return new ColaGroupsLayout(graph, LAYOUT_OPTS);
    case "Concentric":
      return new ConcentricLayout(graph);
    case "BreadthFirst":
      return new BreadthFirstLayout(graph);
    default:
      return new DagreLayout(graph, { ...LAYOUT_OPTS, rankdir: "LR", nodesep: 55, ranksep: 110 });
  }
};

type KialiNodeProps = { element: Node } & WithSelectionProps;

const KialiNode: React.FC<KialiNodeProps> = observer(({ element, onSelect, selected }) => {
  const data = element.getData() as { kind: ReturnType<typeof topologyNodeKind> };
  const Icon = nodeIcon(data.kind);
  return (
    <DefaultNode element={element} showStatusDecorator selected={selected} onSelect={onSelect}>
      <g transform="translate(27, 27)">
        <Icon style={{ color: "#393f44", width: 22, height: 22 } as React.CSSProperties} />
      </g>
    </DefaultNode>
  );
});

const componentFactory: ComponentFactory = ((kind: ModelKind, type: string) => {
  if (type === "group") return DefaultGroup;
  switch (kind) {
    case ModelKind.graph:
      return withPanZoom()(GraphComponent);
    case ModelKind.node:
      return withSelection()(KialiNode);
    case ModelKind.edge:
      return DefaultEdge;
    default:
      return undefined;
  }
}) as ComponentFactory;

function GraphSummary({
  graph,
  selectedNode,
}: {
  graph: TopologyGraph;
  selectedNode: TopologyNode | null;
}): React.ReactElement {
  const apps = graph.nodes.filter((n) => {
    const k = topologyNodeKind(n);
    return k === "app" || k === "workload";
  }).length;
  const services = graph.nodes.filter((n) => {
    const k = topologyNodeKind(n);
    return k === "service" || k === "external";
  }).length;
  const totalFlows = graph.edges.reduce((a, e) => a + e.count, 0);
  const traffic = selectedNode ? trafficOf(graph.edges).get(selectedNode.id) : null;
  const outE = selectedNode
    ? graph.edges.filter((e) => e.source === selectedNode.id).sort((a, b) => b.count - a.count).slice(0, 10)
    : [];
  const inE = selectedNode
    ? graph.edges.filter((e) => e.target === selectedNode.id).sort((a, b) => b.count - a.count).slice(0, 10)
    : [];
  const byId = new Map(graph.nodes.map((n) => [n.id, n]));

  return (
    <div className="nerve-pf-sidebar">
      <Title headingLevel="h3" size="md">
        Current Graph
      </Title>
      <p className="nerve-pf-sidebar-sub">
        {graph.nodes.length} nodes · {graph.edges.length} edges
      </p>
      <div className="nerve-pf-stat-row">
        <div>
          <strong>{apps}</strong>
          <span>Apps</span>
        </div>
        <div>
          <strong>{services}</strong>
          <span>Services</span>
        </div>
        <div>
          <strong>{graph.edges.length}</strong>
          <span>Edges</span>
        </div>
      </div>
      <Divider />
      <Title headingLevel="h4" size="md" className="nerve-pf-section-title">
        Traffic
      </Title>
      <table className="nerve-pf-table">
        <thead>
          <tr>
            <th />
            <th>Total</th>
            <th>% Success</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>Requests / min</td>
            <td className="num">{(totalFlows / 60).toFixed(2)}</td>
            <td className="ok">100.00</td>
          </tr>
        </tbody>
      </table>
      <div className="nerve-pf-health-bar">
        <div className="ok" style={{ width: "100%" }} />
      </div>
      {selectedNode ? (
        <>
          <Divider />
          <Title headingLevel="h4" size="md" className="nerve-pf-section-title">
            {shortTopologyLabel(selectedNode)}
          </Title>
          <p className="nerve-pf-sidebar-sub">
            {topologyNodeKind(selectedNode)} · <code>{selectedNode.id}</code>
          </p>
          <div className="nerve-pf-stat-row nerve-pf-stat-row-sm">
            <div>
              <strong>{traffic?.in ?? 0}</strong>
              <span>Inbound</span>
            </div>
            <div>
              <strong>{traffic?.out ?? 0}</strong>
              <span>Outbound</span>
            </div>
          </div>
          <Title headingLevel="h5" size="md">
            Outbound
          </Title>
          <table className="nerve-pf-table">
            <tbody>
              {outE.length
                ? outE.map((e) => (
                    <tr key={e.source + e.target}>
                      <td>{shortTopologyLabel(byId.get(e.target) ?? { id: e.target, label: e.target, kind: "" })}</td>
                      <td className="num">{e.count}</td>
                    </tr>
                  ))
                : (
                    <tr>
                      <td colSpan={2} className="muted">
                        —
                      </td>
                    </tr>
                  )}
            </tbody>
          </table>
          <Title headingLevel="h5" size="md">
            Inbound
          </Title>
          <table className="nerve-pf-table">
            <tbody>
              {inE.length
                ? inE.map((e) => (
                    <tr key={e.source + e.target}>
                      <td>{shortTopologyLabel(byId.get(e.source) ?? { id: e.source, label: e.source, kind: "" })}</td>
                      <td className="num">{e.count}</td>
                    </tr>
                  ))
                : (
                    <tr>
                      <td colSpan={2} className="muted">
                        —
                      </td>
                    </tr>
                  )}
            </tbody>
          </table>
        </>
      ) : (
        <p className="nerve-pf-hint">
          点击图中节点查看入站 / 出站连接（Kiali 侧栏交互）
        </p>
      )}
    </div>
  );
}

export const NerveTopologyGraph: React.FC<PfTopologyProps> = ({
  graph,
  direction = "all",
  topEdges = 30,
  search = "",
  sourceLabel = "—",
}) => {
  const [layout, setLayout] = React.useState<LayoutName>("Dagre");
  const [layoutOpen, setLayoutOpen] = React.useState(false);
  const [selectedIds, setSelectedIds] = React.useState<string[]>([]);

  const displayGraph = React.useMemo(() => {
    const pruned = pruneServiceMapGraph(graph, direction, topEdges);
    return filterBySearch(pruned, search);
  }, [graph, direction, topEdges, search]);

  const selectedNode = React.useMemo(() => {
    if (!selectedIds.length) return null;
    return displayGraph.nodes.find((n) => n.id === selectedIds[0]) ?? null;
  }, [displayGraph.nodes, selectedIds]);

  const controller = React.useMemo(() => {
    const c = new Visualization();
    c.registerLayoutFactory(layoutFactory);
    c.registerComponentFactory(componentFactory);
    c.addEventListener(GRAPH_LAYOUT_END_EVENT, () => {
      c.getGraph().fit(100);
    });
    c.addEventListener(SELECTION_EVENT, (ids: string[]) => setSelectedIds(ids));
    return c;
  }, []);

  React.useEffect(() => {
    if (!displayGraph.nodes.length) return;
    controller.fromModel(buildModel(displayGraph, layout), true);
  }, [controller, displayGraph, layout]);

  const sideBar = <GraphSummary graph={displayGraph} selectedNode={selectedNode} />;

  const layoutMenu = (
    <Dropdown
      isOpen={layoutOpen}
      popperProps={{ appendTo: () => document.querySelector(".nerve-pf-topology") ?? document.body }}
      toggle={(ref: React.Ref<MenuToggleElement>) => (
        <MenuToggle ref={ref} onClick={() => setLayoutOpen((o) => !o)}>
          Layout: {LAYOUT_LABELS[layout]}
        </MenuToggle>
      )}
    >
      <DropdownList>
        {(Object.keys(LAYOUT_LABELS) as LayoutName[]).map((name) => (
          <DropdownItem
            key={name}
            onClick={() => {
              setLayout(name);
              setLayoutOpen(false);
            }}
          >
            {LAYOUT_LABELS[name]}
          </DropdownItem>
        ))}
      </DropdownList>
    </Dropdown>
  );

  if (!displayGraph.nodes.length) {
    return (
      <div className="nerve-pf-topology nerve-pf-empty">
        <p>暂无拓扑数据</p>
        <p className="nerve-pf-sidebar-sub">来源 {sourceLabel} · 需 flow-relay 或实时会话</p>
      </div>
    );
  }

  return (
    <div className="nerve-pf-topology">
      <TopologyView
        className="nerve-pf-topology-view"
        sideBar={sideBar}
        sideBarOpen
        sideBarResizable
        defaultSideBarSize="300px"
        minSideBarSize="220px"
        maxSideBarSize="480px"
        viewToolbar={
          <ToolbarItem>
            {layoutMenu}
            <span className="nerve-pf-layout-hint muted small">Workloads ← → Services 分组 · 默认 Top 30 连接</span>
          </ToolbarItem>
        }
        controlBar={
          <TopologyControlBar
            controlButtons={createTopologyControlButtons({
              ...defaultControlButtonsOptions,
              zoomInCallback: action(() => controller.getGraph().scaleBy(4 / 3)),
              zoomOutCallback: action(() => controller.getGraph().scaleBy(0.75)),
              fitToScreenCallback: action(() => controller.getGraph().fit(80)),
              resetViewCallback: action(() => {
                controller.getGraph().reset();
                controller.getGraph().layout();
              }),
              legend: false,
            })}
          />
        }
      >
        <VisualizationProvider controller={controller}>
          <VisualizationSurface state={{ selectedIds }} />
        </VisualizationProvider>
      </TopologyView>
    </div>
  );
};
