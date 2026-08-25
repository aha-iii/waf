/** Legacy console module: Advanced security view. */
import { apiGet } from "./api.js";
import { mountPaginatedTable } from "./components/ui.js";

export async function renderAdvancedSecurityView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
  go: (page: "rules-crs" | "rules-ai") => void = () => {},
): Promise<void> {
  root.innerHTML = `
    <div class="page-header"><div><h2>高级安全</h2><p>License、Cluster、Datapath</p></div></div>
    <div class="sync-actions" style="margin-bottom:16px">
      <button type="button" class="ant-btn ant-btn-primary" id="adv-go-crs">打开 CRS 规则集</button>
      <button type="button" class="ant-btn" id="adv-go-ai">打开 AI 规则集</button>
    </div>
    <div class="sync-actions">
      <button type="button" class="ant-btn" id="adv-refresh">刷新</button>
    </div>
    <div class="card"><h3>Datapath</h3><pre class="json-view" id="adv-dp">—</pre></div>
    <div class="card"><h3>Cluster Mesh</h3><div id="adv-clusters">加载中…</div></div>
    <div class="card"><h3>License</h3><pre class="json-view" id="adv-lic">—</pre></div>
    <div class="card"><h3>Mesh</h3><pre class="json-view" id="adv-mesh">—</pre></div>`;

  root.querySelector("#adv-go-crs")?.addEventListener("click", () => go("rules-crs"));
  root.querySelector("#adv-go-ai")?.addEventListener("click", () => go("rules-ai"));

  async function refresh(): Promise<void> {
    const [dp, clusters, lic, mesh] = await Promise.all([
      apiGet<Record<string, unknown>>("/api/datapath/status").catch(() => ({})),
      apiGet<{ clusters: { cluster_id: string; agent_count: number }[] }>("/api/clusters").catch(() => ({
        clusters: [],
      })),
      apiGet<{ usage: unknown }>("/api/license/usage"),
      apiGet<Record<string, unknown>>("/api/mesh/status").catch(() => ({})),
    ]);
    root.querySelector("#adv-dp")!.textContent = JSON.stringify(dp, null, 2);
    const clustersHost = root.querySelector("#adv-clusters")!;
    if (clusters.clusters.length === 0) {
      clustersHost.innerHTML = '<p class="muted">暂无注册集群 — POST /api/clusters/register</p>';
    } else {
      mountPaginatedTable(clustersHost, "adv-clusters", clusters.clusters, {
        headHtml: "<thead><tr><th>Cluster</th><th>Agents</th></tr></thead>",
        rowHtml: (c) => `<tr><td>${c.cluster_id}</td><td>${c.agent_count}</td></tr>`,
      });
    }
    root.querySelector("#adv-lic")!.textContent = JSON.stringify(lic.usage, null, 2);
    root.querySelector("#adv-mesh")!.textContent = JSON.stringify(mesh, null, 2);
  }

  root.querySelector("#adv-refresh")?.addEventListener("click", () => void refresh().catch((e) => toast(String(e), false)));
  await refresh();
}
