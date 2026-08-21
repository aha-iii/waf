/** Bridge from vanilla TS to React PatternFly Topology mount. */
import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { TopologyGraph } from "./network-topology.js";

export type ServiceMapOptions = {
  direction?: "all" | "east_west" | "north_south";
  topEdges?: number;
  search?: string;
  sourceLabel?: string;
};

const roots = new WeakMap<HTMLElement, Root>();

function getHost(container: HTMLElement): HTMLElement {
  return (container.querySelector("#net-topo-pf-root") as HTMLElement | null) ?? container;
}

export function serviceMapShellHtml(): string {
  return `<div id="net-topo-pf-root" class="nerve-pf-mount"></div>`;
}

export async function mountServiceMap(
  container: HTMLElement,
  graph: TopologyGraph,
  opts: ServiceMapOptions = {},
): Promise<void> {
  const host = getHost(container);
  const { NerveTopologyGraph } = await import("./network-topology-pf.js");
  let root = roots.get(host);
  if (!root) {
    root = createRoot(host);
    roots.set(host, root);
  }
  root.render(
    createElement(NerveTopologyGraph, {
      graph,
      direction: opts.direction ?? "all",
      topEdges: opts.topEdges ?? 50,
      search: opts.search ?? "",
      sourceLabel: opts.sourceLabel ?? "—",
    }),
  );
}

export function updateServiceMap(
  container: HTMLElement,
  graph: TopologyGraph,
  opts: ServiceMapOptions = {},
): void {
  void mountServiceMap(container, graph, opts);
}

export function destroyServiceMap(container: HTMLElement): void {
  const host = getHost(container);
  const root = roots.get(host);
  if (root) {
    root.unmount();
    roots.delete(host);
  }
  host.innerHTML = "";
}

export function serviceMapLegendHtml(): string {
  return "";
}
