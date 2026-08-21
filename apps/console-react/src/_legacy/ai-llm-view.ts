/** Legacy (pre-React) console module: Ai llm view. */
import { apiGet, apiPut } from "./api.js";
import { esc, pageHeader } from "./components/ui.js";

type LlmConfigView = {
  enabled: boolean;
  auto_explain: boolean;
  api_url: string;
  model: string;
  api_key_set: boolean;
  llm_ready: boolean;
};

export async function renderAiLlmView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  const res = await apiGet<{ config: LlmConfigView; backend: string }>("/api/triage/llm-config");
  const cfg = res.config;

  root.innerHTML = `
    ${pageHeader("LLM 解释", "")}
    <div class="ant-card" style="margin-top:12px">
      <div class="ant-card-head">LLM 连接</div>
      <div class="ant-card-body form-stack">
        <label><input type="checkbox" id="llm-enabled" ${cfg.enabled ? "checked" : ""} /> 启用 LLM 解释</label>
        <label><input type="checkbox" id="llm-auto" ${cfg.auto_explain ? "checked" : ""} /> inspect 命中后异步预生成 ai_summary</label>
        <label class="ant-form-label">API URL</label>
        <input class="ant-input ant-input-wide" id="llm-url" value="${esc(cfg.api_url)}" />
        <label class="ant-form-label">模型</label>
        <input class="ant-input ant-input-wide" id="llm-model" value="${esc(cfg.model)}" />
        <label class="ant-form-label">API Key ${cfg.api_key_set ? '<span class="ant-tag ant-tag-success">已配置</span>' : '<span class="muted">未配置</span>'}</label>
        <input class="ant-input ant-input-wide" id="llm-key" type="password" placeholder="${cfg.api_key_set ? "留空保持不变" : "sk-..."}" autocomplete="off" />
        <p class="muted small">${cfg.llm_ready ? "LLM 就绪" : "未就绪"} · ${esc(res.backend)}</p>
        <button type="button" class="ant-btn ant-btn-primary" id="btn-save-llm">保存 LLM 配置</button>
      </div>
    </div>`;

  root.querySelector("#btn-save-llm")?.addEventListener("click", async () => {
    const body: Record<string, unknown> = {
      enabled: (root.querySelector("#llm-enabled") as HTMLInputElement).checked,
      auto_explain: (root.querySelector("#llm-auto") as HTMLInputElement).checked,
      api_url: (root.querySelector("#llm-url") as HTMLInputElement).value.trim(),
      model: (root.querySelector("#llm-model") as HTMLInputElement).value.trim(),
    };
    const key = (root.querySelector("#llm-key") as HTMLInputElement).value.trim();
    if (key) body.api_key = key;
    try {
      await apiPut("/api/triage/llm-config", body);
      toast("LLM 配置已保存");
      await renderAiLlmView(root, toast);
    } catch (e) {
      toast(String(e), false);
    }
  });
}
