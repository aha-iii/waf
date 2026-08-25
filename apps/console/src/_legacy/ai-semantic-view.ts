/** Legacy console module: Ai semantic view. */
import { apiGet, apiPost, apiPut } from "./api.js";
import { pageHeader, runMutatingAction } from "./components/ui.js";
import { GLOBAL_DEFAULT_SITE_ID } from "./sites.js";
import {
  collectSemanticConfig,
  bindSemanticDecodeOrder,
  mergeSemanticConfig,
  renderSemanticConsole,
  type SemanticConfig,
  type SemanticStats,
  type SemanticTab,
} from "./semantic-config-ui.js";

function siteQ(siteId: string): string {
  return `?site_id=${encodeURIComponent(siteId)}`;
}

export async function renderAiSemanticView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
  _initialSiteId = GLOBAL_DEFAULT_SITE_ID,
): Promise<void> {
  const sid = GLOBAL_DEFAULT_SITE_ID;
  let activeTab: SemanticTab = "general";

  const [res, statsRes] = await Promise.all([
    apiGet<{ config: Partial<SemanticConfig>; backend: string }>("/api/global/semantic/config"),
    apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`).catch(() => null),
  ]);

  let cfg = mergeSemanticConfig(res.config);
  let stats = statsRes;
  const backend = res.backend;

  function render(): void {
    root.innerHTML = `
      ${pageHeader("语义增强", "")}
      ${renderSemanticConsole({
        activeTab,
        cfg,
        stats,
        backend,
        toolbarLeft: `<span class="muted">全局默认</span>`,
        toolbarRight: `<button type="button" class="ant-btn" id="btn-refresh-stats">刷新观测</button>
          <button type="button" class="ant-btn ant-btn-primary" id="btn-save-sem">保存配置</button>`,
        footer: `持久化 <strong>${backend}</strong>`,
      })}`;

    bindNav();
    bindPanel();
    bindActions();
  }

  function syncFromForm(): void {
    cfg = collectSemanticConfig(root, cfg);
  }

  function bindNav(): void {
    root.querySelectorAll<HTMLButtonElement>("[data-sem-tab]").forEach((btn) => {
      btn.onclick = () => {
        syncFromForm();
        activeTab = (btn.getAttribute("data-sem-tab") as SemanticTab) || "general";
        render();
      };
    });
  }

  function bindPanel(): void {
    bindSemanticDecodeOrder(root, syncFromForm);
  }

  function bindActions(): void {
    root.querySelector("#btn-save-sem")?.addEventListener("click", () => {
      syncFromForm();
      void runMutatingAction({
        action: "save",
        subject: "全局默认语义配置",
        confirmMessage: "确定保存全局默认语义配置吗？",
        successMessage: "全局默认语义配置已保存",
        run: async () => {
          await apiPut("/api/global/semantic/config", cfg);
        },
      });
    });
    root.querySelector("#btn-refresh-stats")?.addEventListener("click", async () => {
      try {
        stats = await apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`);
        activeTab = "observe";
        render();
        toast("观测数据已刷新");
      } catch (e) {
        toast(String(e), false);
      }
    });

    root.querySelector("#btn-fb")?.addEventListener("click", async () => {
      try {
        const resp = await apiPost<{ config_updated?: boolean; auto_tune?: { actions?: string[] } }>(
          "/api/semantic/feedback",
          {
            site_id: sid,
            event_id: (root.querySelector("#fb-event") as HTMLInputElement).value || undefined,
            verdict: (root.querySelector("#fb-verdict") as HTMLSelectElement).value,
            note: (root.querySelector("#fb-note") as HTMLInputElement).value || undefined,
          },
        );
        stats = await apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`).catch(() => stats);
        if (resp.config_updated) {
          const cfgRes = await apiGet<{ config: Partial<SemanticConfig> }>(
            `/api/semantic/config${siteQ(sid)}`,
          );
          cfg = mergeSemanticConfig(cfgRes.config);
        }
        toast(
          resp.config_updated
            ? `反馈已记录，已自动调优: ${resp.auto_tune?.actions?.join(", ") ?? ""}`
            : "反馈已记录",
        );
        render();
      } catch (e) {
        toast(String(e), false);
      }
    });
  }

  render();
}
