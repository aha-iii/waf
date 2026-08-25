/** Legacy console module: Rules ai view. */
import { apiGet, apiPost } from "./api.js";
import { bindPaginationBar, categoryLabel, defaultPaginationState, esc, renderPaginatedSection, type PaginationState } from "./components/ui.js";

type PackSummary = {
  pack_id: string;
  version: string;
  description?: string;
  enabled: boolean;
  rule_count: number;
};

type RuleSummary = {
  id: string;
  name: string;
  category: string;
  severity: string;
  source: string;
};

const CATEGORY_HELP: Record<string, string> = {
  prompt_injection: "Prompt 注入 / 越狱 — 针对 LLM 应用 API",
  ai_artifact: "AI 助手配置泄露 — .cursor/.claude 等目录访问",
};

export async function renderRulesAiView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  root.innerHTML = `
    <div class="page-header">
      <div>
        <h2>AI 应用安全</h2>
      </div>
    </div>
    <div class="sync-actions">
      <button type="button" class="ant-btn ant-btn-primary" id="ai-reload">热加载规则</button>
      <button type="button" class="ant-btn" id="ai-refresh">刷新</button>
    </div>
    <div class="card"><h3>规则包</h3><div id="ai-packs">加载中…</div></div>
    <div class="card">
      <h3>规则明细</h3>
      <div id="ai-rules">加载中…</div>
    </div>`;

  const packsPage: PaginationState = defaultPaginationState();
  const rulesPage: PaginationState = defaultPaginationState();

  function paintPacks(packs: PackSummary[]): void {
    const host = root.querySelector("#ai-packs")!;
    if (!packs.length) {
      host.innerHTML = `<p class="muted">暂无规则包 — 请将 JSON 放入 <code>rules/ai/</code></p>`;
      return;
    }
    const section = renderPaginatedSection("ai-packs", packs, packsPage, (pagePacks) => {
      const rows = pagePacks
        .map(
          (p) => `<tr>
            <td><code>${esc(p.pack_id)}</code></td>
            <td>${esc(p.version)}</td>
            <td>${p.enabled ? "启用" : "禁用"}</td>
            <td>${p.rule_count}</td>
            <td class="muted">${esc(p.description || "—")}</td>
          </tr>`,
        )
        .join("");
      return `<table class="data-table">
        <thead><tr><th>包 ID</th><th>版本</th><th>状态</th><th>规则数</th><th>说明</th></tr></thead>
        <tbody>${rows}</tbody></table>`;
    });
    packsPage.page = section.page;
    host.innerHTML = section.html;
    bindPaginationBar(host, "ai-packs", packsPage, packs.length, (next) => {
      packsPage.page = next.page;
      packsPage.pageSize = next.pageSize;
      paintPacks(packs);
    });
  }

  function paintRules(aiRules: RuleSummary[]): void {
    const host = root.querySelector("#ai-rules")!;
    if (!aiRules.length) {
      host.innerHTML = `<div class="ant-empty">暂无规则 — 点击「热加载规则」</div>`;
      return;
    }
    const section = renderPaginatedSection("ai-rules", aiRules, rulesPage, (pageRules) => {
      const rows = pageRules
        .map(
          (r) => `<tr>
            <td><code>${esc(r.id)}</code></td>
            <td>${esc(r.name)}</td>
            <td>${categoryLabel(r.category)}<br><span class="muted small">${esc(CATEGORY_HELP[r.category] ?? "")}</span></td>
            <td>${esc(r.severity)}</td>
          </tr>`,
        )
        .join("");
      return `<table class="data-table">
        <thead><tr><th>ID</th><th>名称</th><th>分类</th><th>严重度</th></tr></thead>
        <tbody>${rows}</tbody></table>`;
    });
    rulesPage.page = section.page;
    host.innerHTML = section.html;
    bindPaginationBar(host, "ai-rules", rulesPage, aiRules.length, (next) => {
      rulesPage.page = next.page;
      rulesPage.pageSize = next.pageSize;
      paintRules(aiRules);
    });
  }

  async function refresh(): Promise<void> {
    const inv = await apiGet<{
      ai_packs: PackSummary[];
      rules: RuleSummary[];
      rules_dir?: string;
    }>("/api/rules/packs?limit=500");

    const aiRules = (inv.rules || []).filter(
      (r) => r.source.includes("/ai/") || r.category.startsWith("prompt") || r.category.startsWith("ai"),
    );

    paintPacks(inv.ai_packs ?? []);
    paintRules(aiRules);
  }

  root.querySelector("#ai-reload")?.addEventListener("click", async () => {
    try {
      const r = await apiPost<{ rules: number }>("/api/rules/packs", {});
      toast(`已热加载 ${r.rules} 条 L7 规则`);
      await refresh();
    } catch (e) {
      toast(String(e), false);
    }
  });

  root.querySelector("#ai-refresh")?.addEventListener("click", () =>
    void refresh().catch((e) => toast(String(e), false)),
  );

  await refresh();
}
