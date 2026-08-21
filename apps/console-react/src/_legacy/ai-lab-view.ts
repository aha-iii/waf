/** Legacy (pre-React) console module: Ai lab view. */
import { apiGet, apiPost } from "./api.js";
import { esc, pageHeader } from "./components/ui.js";

export async function renderAiLabView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  const cfg = await apiGet<Record<string, unknown>>("/api/triage/config");

  root.innerHTML = `
    ${pageHeader("研判实验室", "")}
    <div class="ant-card">
      <div class="ant-card-head">引擎状态</div>
      <div class="ant-card-body">
        <ul class="muted">
          <li>研判引擎：<code>${esc(String(cfg.triage_engine ?? "multi_signal_fusion_v1"))}</code></li>
          <li>语义检测：${cfg.semantic_enabled ? "启用" : "关闭"}</li>
          <li>配置存储：<code>${esc(String(cfg.settings_backend ?? "-"))}</code></li>
          <li>LLM：${cfg.llm_ready ? `就绪 (${esc(String(cfg.llm_model ?? ""))})` : "template_v1 回退"}</li>
        </ul>
      </div>
    </div>
    <div class="ant-card" style="margin-top:12px">
      <div class="ant-card-head">快速测试</div>
      <div class="ant-card-body form-stack">
        <label class="ant-form-label">测试 URI</label>
        <input class="ant-input ant-input-wide" id="lab-uri" value="/?id=%2527%2520OR%25201%253D1" />
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <button type="button" class="ant-btn" id="btn-lab-inspect">inspect 检测</button>
          <button type="button" class="ant-btn ant-btn-primary" id="btn-lab-explain">研判 + AI 解释</button>
        </div>
        <pre id="lab-out" class="muted small" style="white-space:pre-wrap;max-height:360px;overflow:auto">—</pre>
      </div>
    </div>`;

  const out = root.querySelector("#lab-out") as HTMLElement;

  root.querySelector("#btn-lab-inspect")?.addEventListener("click", async () => {
    const uri = (root.querySelector("#lab-uri") as HTMLInputElement).value;
    out.textContent = "请求中…";
    try {
      const r = await apiPost<Record<string, unknown>>("/api/inspect", { uri, persist: false });
      out.textContent = JSON.stringify(r, null, 2);
    } catch (e) {
      out.textContent = String(e);
      toast(String(e), false);
    }
  });

  root.querySelector("#btn-lab-explain")?.addEventListener("click", async () => {
    const uri = (root.querySelector("#lab-uri") as HTMLInputElement).value;
    out.textContent = "研判中…";
    try {
      const inspect = await apiPost<{ hit?: Record<string, unknown>; triage?: Record<string, unknown> }>(
        "/api/inspect",
        { uri, persist: false },
      );
      const event = {
        id: "lab-" + Date.now(),
        ts: new Date().toISOString(),
        layer: "l7",
        action: inspect.hit ? "block" : "allow",
        category: String((inspect.hit as { category?: string })?.category ?? "sqli"),
        severity: String((inspect.hit as { severity?: string })?.severity ?? "medium"),
        uri,
        method: "GET",
        payload_sample: String((inspect.hit as { payload_sample?: string })?.payload_sample ?? ""),
      };
      const explain = await apiPost<Record<string, unknown>>("/api/triage/explain", { event });
      out.textContent = JSON.stringify({ inspect: { blocked: !(inspect as { allowed?: boolean }).allowed, triage: inspect.triage }, explain }, null, 2);
    } catch (e) {
      out.textContent = String(e);
      toast(String(e), false);
    }
  });
}
