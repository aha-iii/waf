/** Legacy console module: Access view. */
import { apiGet, apiPost, apiPut, apiTempAllow, apiTempDeny, apiListRuntimeDenies } from "./api.js";
import { btn, card, esc, formField, formStack, input, mountPaginatedTable, pageHeader, tabBar } from "./components/ui.js";

export async function renderAccessView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  root.innerHTML = `
    ${pageHeader("访问控制", "")}
    ${tabBar([
      { id: "blacklist", label: "IP 黑名单", active: true },
      { id: "runtime", label: "临时封禁" },
      { id: "quick", label: "应急封禁" },
    ])}
    <div id="access-panel"></div>`;

  const panel = root.querySelector("#access-panel")!;

  async function showBlacklist(): Promise<void> {
    const data = await apiGet<{ total: number; entries: { ip: string; kind: string; source?: string }[] }>(
      "/api/policy/blacklist",
    );
    panel.innerHTML = card({
      title: "策略黑名单",
      tip: `共 ${data.total} 条`,
      body: `<div id="bl-table"></div>`,
    });
    mountPaginatedTable(panel.querySelector("#bl-table")!, "access-bl", data.entries, {
      tableClass: "ant-table",
      headHtml: "<thead><tr><th>IP</th><th>类型</th><th>来源</th></tr></thead>",
      rowHtml: (e) => `<tr><td><code>${esc(e.ip)}</code></td><td>${esc(e.kind)}</td><td>${esc(e.source ?? "—")}</td></tr>`,
      emptyText: "暂无永久黑名单",
    });
  }

  async function showRuntime(): Promise<void> {
    const data = await apiListRuntimeDenies();
    panel.innerHTML = card({
      title: "运行时临时封禁",
      tip: `${data.total} 条`,
      body: `<div id="runtime-table"></div>`,
    });
    mountPaginatedTable(panel.querySelector("#runtime-table")!, "access-rt", data.denies, {
      tableClass: "ant-table",
      headHtml: "<thead><tr><th>IP</th><th>来源</th><th>原因</th><th>过期</th><th></th></tr></thead>",
      rowHtml: (d) => `<tr>
        <td><code>${esc(d.ip)}</code></td><td>${esc(d.source)}</td><td>${esc(d.reason ?? "—")}</td>
        <td>${esc(d.expires_at?.slice(0, 19) ?? "—")}</td>
        <td>${btn({ label: "解封", size: "sm", className: "btn-unblock", attrs: { "data-ip": d.ip } })}</td>
      </tr>`,
      emptyText: "暂无临时封禁",
      onAfterRender: () => {
        panel.querySelectorAll(".btn-unblock").forEach((b) => {
          b.addEventListener("click", async () => {
            const ip = (b as HTMLElement).dataset.ip!;
            await apiTempAllow(ip);
            toast(`已解封 ${ip}`);
            await showRuntime();
          });
        });
      },
    });
  }

  function showQuick(): void {
    panel.innerHTML = card({
      title: "应急封禁 IP",
      body: formStack(`
        ${formField("IPv4 地址", input({ id: "block-ip", wide: true, placeholder: "203.0.113.50" }))}
        ${formField("原因（可选）", input({ id: "block-reason", wide: true, placeholder: "手动封禁" }))}
        <div class="form-actions">
          ${btn({ id: "btn-block-temp", label: "临时封禁", variant: "primary" })}
          ${btn({ id: "btn-block-playbook", label: "Playbook 永久封禁" })}
        </div>`),
    });
    panel.querySelector("#btn-block-temp")?.addEventListener("click", async () => {
      const ip = (panel.querySelector("#block-ip") as HTMLInputElement).value.trim();
      if (!ip) return toast("请输入 IP", false);
      await apiTempDeny(ip, "console");
      toast(`已临时封禁 ${ip}`);
    });
    panel.querySelector("#btn-block-playbook")?.addEventListener("click", async () => {
      const ip = (panel.querySelector("#block-ip") as HTMLInputElement).value.trim();
      const reason = (panel.querySelector("#block-reason") as HTMLInputElement).value.trim();
      if (!ip) return toast("请输入 IP", false);
      await apiPost("/api/playbook/block-ip", { ip, reason: reason || undefined });
      toast(`Playbook 已封禁 ${ip}`);
    });
  }

  const tabs: Record<string, () => void | Promise<void>> = {
    blacklist: showBlacklist,
    runtime: showRuntime,
    quick: showQuick,
  };

  root.querySelectorAll<HTMLElement>(".tab-btn").forEach((b) => {
    b.addEventListener("click", () => {
      root.querySelectorAll(".tab-btn").forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      const fn = tabs[b.dataset.tab!];
      void Promise.resolve(fn()).catch((e) => toast(String(e), false));
    });
  });

  await showBlacklist();
}
