/** Legacy (pre-React) console module: Intel view. */
import { apiGet, apiPost, apiPut } from "./api.js";
import {
  btn,
  card,
  formStack,
  input,
  mountPaginatedTable,
  pageHeader,
  tableToolbar,
} from "./components/ui.js";

type IntelEntry = { ip: string; category: string; severity?: string; note?: string };

export async function renderIntelView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const res = await apiGet<{ entries: IntelEntry[] }>("/api/intel/feed");
  let entries = res.entries ?? [];

  root.innerHTML = `
    ${pageHeader("威胁情报", "")}
    ${tableToolbar(
      btn({ id: "btn-add-intel", label: "+ 添加 IP" }),
      btn({ id: "btn-save-intel", label: "保存情报库", variant: "primary" }),
    )}
    <div class="dash-grid-2">
      ${card({
        title: "情报条目",
        tipHtml: `<span class="head-tip" id="intel-count">${entries.length} 条</span>`,
        bodyId: "intel-table",
        body: "",
      })}
      ${card({
        title: "情报查询",
        body: formStack(
          `${input({ id: "intel-lookup-ip", wide: true, placeholder: "1.2.3.4" })}
           ${btn({ id: "btn-intel-lookup", label: "查询" })}
           <pre class="json-view" id="intel-lookup-out">—</pre>`,
        ),
      })}
    </div>`;

  function renderTable(): void {
    const el = root.querySelector("#intel-table")!;
    root.querySelector("#intel-count")!.textContent = `${entries.length} 条`;
    mountPaginatedTable(el, "intel", entries, {
      tableClass: "ant-table",
      headHtml: "<thead><tr><th>IP</th><th>类别</th><th>严重度</th><th>备注</th><th></th></tr></thead>",
      rowHtml: (e, i) => `<tr>
        <td>${input({ sm: true, value: e.ip, attrs: { "data-i": String(i), "data-f": "ip" } })}</td>
        <td>${input({ sm: true, value: e.category, attrs: { "data-i": String(i), "data-f": "category" } })}</td>
        <td>${input({ sm: true, value: e.severity ?? "high", attrs: { "data-i": String(i), "data-f": "severity" } })}</td>
        <td>${input({ value: e.note ?? "", attrs: { "data-i": String(i), "data-f": "note" } })}</td>
        <td>${btn({ label: "删除", variant: "link", className: "danger btn-del", attrs: { "data-i": String(i) } })}</td>
      </tr>`,
      emptyText: "暂无情报，点击「添加 IP」",
      onAfterRender: () => {
        el.querySelectorAll("input[data-f]").forEach((inp) => {
          inp.addEventListener("change", () => {
            const idx = Number((inp as HTMLElement).dataset.i);
            const f = (inp as HTMLElement).dataset.f! as keyof IntelEntry;
            (entries[idx] as unknown as Record<string, string>)[f] = (inp as HTMLInputElement).value;
          });
        });
        el.querySelectorAll(".btn-del").forEach((b) => {
          b.addEventListener("click", () => {
            entries.splice(Number((b as HTMLElement).dataset.i), 1);
            renderTable();
          });
        });
      },
    });
  }

  root.querySelector("#btn-add-intel")?.addEventListener("click", () => {
    entries.push({ ip: "0.0.0.0", category: "malware", severity: "high", note: "" });
    renderTable();
  });

  root.querySelector("#btn-save-intel")?.addEventListener("click", async () => {
    await apiPut("/api/intel/feed", entries);
    toast("威胁情报已保存");
  });

  root.querySelector("#btn-intel-lookup")?.addEventListener("click", async () => {
    const ip = (root.querySelector("#intel-lookup-ip") as HTMLInputElement).value.trim();
    const r = await apiPost("/api/intel/lookup", { ip });
    root.querySelector("#intel-lookup-out")!.textContent = JSON.stringify(r, null, 2);
  });

  renderTable();
}
