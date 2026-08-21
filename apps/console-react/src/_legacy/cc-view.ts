/** Legacy (pre-React) console module: Cc view. */
import { apiGet, apiPut } from "./api.js";

type CcConfig = {
  enabled: boolean;
  requests_per_minute: number;
  burst: number;
  block_duration_sec: number;
};

export async function renderCcView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const res = await apiGet<{ config: CcConfig }>("/api/cc/config");
  const cfg = res.config;
  root.innerHTML = `
    <div class="page-header"><div><h2>CC 防护</h2><p>按 IP + Host + URI 令牌桶限流</p></div></div>
    <div class="card">
      <label><input type="checkbox" id="cc-enabled" ${cfg.enabled ? "checked" : ""} /> 启用 CC 防护</label>
      <p class="muted">每分钟请求数</p>
      <input type="number" id="cc-rpm" class="ant-input" value="${cfg.requests_per_minute}" />
      <p class="muted">突发容量</p>
      <input type="number" id="cc-burst" class="ant-input" value="${cfg.burst}" />
      <p class="muted">封禁时长（秒）</p>
      <input type="number" id="cc-block" class="ant-input" value="${cfg.block_duration_sec}" />
      <div style="margin-top:12px"><button type="button" class="ant-btn ant-btn-primary" id="btn-save-cc">保存</button></div>
    </div>`;

  root.querySelector("#btn-save-cc")?.addEventListener("click", async () => {
    await apiPut("/api/cc/config", {
      enabled: (root.querySelector("#cc-enabled") as HTMLInputElement).checked,
      requests_per_minute: Number((root.querySelector("#cc-rpm") as HTMLInputElement).value),
      burst: Number((root.querySelector("#cc-burst") as HTMLInputElement).value),
      block_duration_sec: Number((root.querySelector("#cc-block") as HTMLInputElement).value),
    });
    toast("CC 配置已保存");
  });
}
