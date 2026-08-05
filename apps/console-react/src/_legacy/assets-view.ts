import { openAppConfigDrawer } from "./app-config-drawer.js";
import { apiGet, apiPut } from "./api.js";
import { esc, pageHeader, showConfirm } from "./components/ui.js";
import { renderSiteCapabilityTags } from "./module-display.js";
import type { ProtectionModules } from "./security-view.js";
import { loadSites, navigateToAppEvents, type ProtectedSite } from "./sites.js";

export async function renderAssetsView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  root.innerHTML = `
    ${pageHeader("防护应用", "")}
    <div class="table-toolbar">
      <button type="button" class="ant-btn" id="btn-assets-refresh">刷新</button>
      <button type="button" class="ant-btn ant-btn-primary" id="btn-assets-add">+ 新增应用</button>
    </div>
    <div id="assets-body">加载中…</div>`;

  const body = root.querySelector("#assets-body")!;
  let sites: ProtectedSite[] = [];
  let modulesBySite: Record<string, ProtectionModules> = {};

  async function loadModulesSummary(): Promise<void> {
    modulesBySite = {};
    await Promise.all(
      sites.map(async (s) => {
        try {
          modulesBySite[s.id] = await apiGet<ProtectionModules>(
            `/api/modules?site_id=${encodeURIComponent(s.id)}`,
          );
        } catch {
          modulesBySite[s.id] = { version: 1, mode: "block", modules: {} };
        }
      }),
    );
  }

  function openConfig(index: number): void {
    if (index < 0 || index >= sites.length) return;
    void openAppConfigDrawer({
      site: { ...sites[index] },
      sites,
      siteIndex: index,
      onSitesChange: (next) => {
        sites = next;
      },
      onSaved: () => {
        void loadSites().then(async (list) => {
          sites = list;
          await loadModulesSummary();
          render();
        });
      },
      toast,
    });
  }

  async function deleteSite(index: number): Promise<void> {
    const s = sites[index];
    if (s.id === "default") return;
    const ok = await showConfirm({
      title: "删除应用",
      message: `删除应用「${s.name.trim() || s.id}」？`,
      okText: "删除",
      danger: true,
    });
    if (!ok) return;
    sites.splice(index, 1);
    try {
      await apiPut("/api/sites", sites);
      toast("已删除");
      sites = await loadSites();
      await loadModulesSummary();
      render();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
    }
  }

  function render(): void {
    const empty =
      sites.length === 0
        ? `<div class="ant-empty" style="padding:48px">暂无应用</div>`
        : "";

    body.innerHTML = `
      <div class="asset-summary-bar">
        <span>共 <strong>${sites.length}</strong> 个应用</span>
        <span><strong>${sites.filter((s) => s.enabled).length}</strong> 个防护中</span>
      </div>
      ${empty}
      <div class="asset-grid" id="assets-cards"></div>`;

    const cards = body.querySelector("#assets-cards");
    if (!cards || !sites.length) return;

    cards.innerHTML = sites
      .map((s, i) => {
        const mod = modulesBySite[s.id] ?? { version: 1, mode: "block", modules: {} };
        const capTags = renderSiteCapabilityTags(mod, esc);
        return `<div class="asset-card ${s.enabled ? "protected" : "paused"}">
          <span class="asset-status-tag ant-tag ${s.enabled ? "ant-tag-success" : ""}">${s.enabled ? "防护中" : "已暂停"}</span>
          <div class="asset-card-head">
            <div class="asset-icon">${s.enabled ? "🛡" : "⏸"}</div>
            <div><strong>${esc(s.name)}</strong><div class="muted small">${esc(s.id)}</div></div>
          </div>
          <div class="asset-card-body">
            <div class="asset-field"><span class="lbl">域名</span><span>${esc(s.hosts.join(", "))}</span></div>
            ${s.description || s.upstream ? `<div class="asset-field"><span class="lbl">备注</span><code class="small">${esc(s.description ?? s.upstream ?? "")}</code></div>` : ""}
            <div class="asset-field"><span class="lbl">模块</span>
              <span class="cap-tags module-cap-tags">${capTags}</span>
            </div>
            <div class="asset-card-actions">
              <div class="asset-card-actions-left">
                <button type="button" class="ant-btn ant-btn-sm ant-btn-primary" data-goto-config="${i}">配置应用</button>
                ${
                  s.id === "default"
                    ? ""
                    : `<button type="button" class="ant-btn ant-btn-sm ant-btn-danger" data-del="${i}">删除</button>`
                }
              </div>
              <div class="asset-card-actions-right">
                <button type="button" class="ant-btn ant-btn-sm asset-events-btn" data-goto-events="${esc(s.id)}" title="查看本应用攻击记录">攻击记录</button>
              </div>
            </div>
          </div>
        </div>`;
      })
      .join("");

    cards.querySelectorAll("[data-goto-events]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        navigateToAppEvents((btn as HTMLElement).dataset.gotoEvents!);
      });
    });
    cards.querySelectorAll("[data-goto-config]").forEach((btn) => {
      btn.addEventListener("click", () => openConfig(Number((btn as HTMLElement).dataset.gotoConfig)));
    });
    cards.querySelectorAll("[data-del]").forEach((btn) => {
      btn.addEventListener("click", () => {
        void deleteSite(Number((btn as HTMLElement).dataset.del));
      });
    });
  }

  root.querySelector("#btn-assets-add")?.addEventListener("click", () => {
    const id = `app-${Date.now().toString(36)}`;
    sites.push({
      id,
      name: "",
      hosts: [],
      enabled: true,
    });
    openConfig(sites.length - 1);
  });

  root.querySelector("#btn-assets-refresh")?.addEventListener("click", () => {
    void (async () => {
      sites = await loadSites();
      await loadModulesSummary();
      render();
      toast("已刷新");
    })();
  });

  try {
    sites = await loadSites();
    await loadModulesSummary();
    render();
  } catch (e) {
    body.innerHTML = `<div class="ant-alert ant-alert-error">${e instanceof Error ? e.message : String(e)}</div>`;
    toast(String(e), false);
  }
}
