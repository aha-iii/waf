import { apiGet, apiPost } from "./api.js";
import { consumeCrsViewPreset } from "./crs-preset.js";
import {
  bindRiskSectionsPagination,
  CATEGORY_ORDER,
  fetchRuleRows,
  normalizeCategory,
  PROTECT_LABEL,
  renderRiskSectionsHtml,
  rowsForCategory,
  ruleAtProtectLevel,
  sensitivityToProtect,
  type CrsVersion,
  type ModuleConfig,
  type PackSummary,
  type ProtectLevel,
  type RuleRow,
  type RulesInventory,
} from "./crs-rules-shared.js";
import { categoryLabel, esc, showConfirm, bindPaginationBar, defaultPaginationState, renderPaginatedSection, type PaginationState } from "./components/ui.js";

function packTable(packs: PackSummary[], page: PaginationState): { html: string; page: number } {
  if (!packs.length) return { html: `<p class="muted">暂无规则包</p>`, page: 1 };
  return renderPaginatedSection("crs-packs", packs, page, (pagePacks) => {
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
}

export async function renderRulesCrsView(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
): Promise<void> {
  root.innerHTML = `
    <div class="page-header"><div><h2>核心规则库</h2></div></div>
    <div class="sync-actions">
      <button type="button" class="ant-btn ant-btn-primary" id="crs-update">更新最新规则集</button>
    </div>
    <div class="dash-grid" style="grid-template-columns:repeat(4,minmax(0,1fr));margin-bottom:16px" id="crs-stats"></div>
    <div class="card"><h3>当前规则包</h3><div id="crs-packs">加载中…</div></div>
    <div class="card"><h3>版本历史</h3><div id="crs-versions">加载中…</div></div>
    <div class="card">
      <h3>规则分级一览</h3>
      <div class="table-toolbar" style="flex-wrap:wrap;gap:12px;margin-bottom:8px">
        <div class="category-tabs" id="crs-cat-tabs"></div>
        <input class="ant-input" id="crs-rule-filter" placeholder="搜索 ID / 名称" style="width:220px;margin-left:auto" />
      </div>
      <div class="protect-level-bar">
        <span class="protect-level-label">拦截等级</span>
        <div class="protect-level-tabs" id="crs-protect-tabs"></div>
      </div>
      <div id="crs-tier-rules">加载中…</div>
    </div>`;

  let allRows: RuleRow[] = [];
  let moduleConfigs: Record<string, ModuleConfig> = {};
  const viewPreset = consumeCrsViewPreset();
  let activeCategory = viewPreset?.category ?? "sqli";
  let activeProtect: ProtectLevel = viewPreset?.protect ?? 1;
  const presetLockedProtect = viewPreset?.protect != null;
  const pageState: Record<string, { page: number; pageSize: number }> = {};
  const packsPage: PaginationState = defaultPaginationState();
  const versionsPage: PaginationState = defaultPaginationState();
  let cachedVersions: CrsVersion[] = [];
  let cachedPacks: PackSummary[] = [];
  let cachedCrsRules = 0;
  let cachedMaxVersions = 10;

  function paintPacks(): void {
    const packsHost = root.querySelector("#crs-packs")!;
    const packSection = packTable(cachedPacks, packsPage);
    packsPage.page = packSection.page;
    packsHost.innerHTML = `<p class="muted">CRS 规则包 ${cachedPacks.length} 个 · 共 ${cachedCrsRules} 条</p>${packSection.html}`;
    bindPaginationBar(packsHost, "crs-packs", packsPage, cachedPacks.length, (next) => {
      packsPage.page = next.page;
      packsPage.pageSize = next.pageSize;
      paintPacks();
    });
  }

  function paintVersions(): void {
    const host = root.querySelector("#crs-versions")!;
    if (!cachedVersions.length) {
      host.innerHTML = `<p class="muted">暂无历史版本 — 更新或「保存快照」后会自动创建</p>`;
      return;
    }
    const section = renderPaginatedSection("crs-versions", cachedVersions, versionsPage, (pageVersions) => {
      const rows = pageVersions
        .map(
          (v) => `<tr>
            <td><code>${esc(v.id)}</code></td>
            <td>${esc(new Date(v.created_at).toLocaleString("zh-CN"))}</td>
            <td>${v.pack_count}</td>
            <td>${v.rule_count}</td>
            <td class="muted">${esc(v.note || v.crs_version || "—")}</td>
            <td><button type="button" class="ant-btn ant-btn-sm crs-rollback" data-id="${esc(v.id)}">回滚</button></td>
          </tr>`,
        )
        .join("");
      return `<table class="data-table">
        <thead><tr><th>版本 ID</th><th>时间</th><th>包数</th><th>规则数</th><th>说明</th><th></th></tr></thead>
        <tbody>${rows}</tbody></table>`;
    });
    versionsPage.page = section.page;
    host.innerHTML = `<p class="muted">保留最近 ${cachedMaxVersions} 次 · 当前 ${cachedVersions.length} 条记录</p>${section.html}`;
    bindPaginationBar(host, "crs-versions", versionsPage, cachedVersions.length, (next) => {
      versionsPage.page = next.page;
      versionsPage.pageSize = next.pageSize;
      paintVersions();
    });
    host.querySelectorAll<HTMLButtonElement>(".crs-rollback").forEach((btn) => {
      btn.onclick = () => void rollback(btn.dataset.id || "");
    });
  }

  function renderCategoryTabs(): void {
    const counts = new Map<string, number>();
    for (const r of allRows) {
      const c = normalizeCategory(r.category);
      counts.set(c, (counts.get(c) ?? 0) + 1);
    }
    const tabs = CATEGORY_ORDER.filter((c) => (counts.get(c) ?? 0) > 0)
      .concat([...counts.keys()].filter((c) => !CATEGORY_ORDER.includes(c as never)))
      .map((c) => {
        const n = counts.get(c) ?? 0;
        const active = c === activeCategory ? " active" : "";
        return `<button type="button" class="category-tab${active}" data-cat="${esc(c)}">${categoryLabel(c)} <span class="muted">(${n})</span></button>`;
      })
      .join("");
    const el = root.querySelector("#crs-cat-tabs")!;
    el.innerHTML = tabs || `<span class="muted">暂无规则</span>`;
    el.querySelectorAll<HTMLButtonElement>(".category-tab").forEach((btn) => {
      btn.onclick = () => {
        activeCategory = btn.dataset.cat || "sqli";
        const mod = moduleConfigs[activeCategory];
        if (mod && !presetLockedProtect) activeProtect = sensitivityToProtect(mod.sensitivity);
        renderCategoryTabs();
        renderProtectTabs();
        renderRiskSections((root.querySelector("#crs-rule-filter") as HTMLInputElement).value);
      };
    });
  }

  function renderProtectTabs(): void {
    const catRows = rowsForCategory(allRows, activeCategory);
    const counts: Record<ProtectLevel, number> = { 1: 0, 2: 0, 3: 0 };
    for (const p of [1, 2, 3] as ProtectLevel[]) {
      counts[p] = catRows.filter((r) => ruleAtProtectLevel(r, p)).length;
    }

    const tabs = ([1, 2, 3] as ProtectLevel[])
      .map((p) => {
        const active = p === activeProtect ? " active" : "";
        const n = counts[p];
        return `<button type="button" class="protect-level-tab${active}" data-protect="${p}">${PROTECT_LABEL[p]} <span class="muted">(${n})</span></button>`;
      })
      .join("");
    root.querySelector("#crs-protect-tabs")!.innerHTML = tabs;

    root.querySelectorAll<HTMLButtonElement>(".protect-level-tab").forEach((btn) => {
      btn.onclick = () => {
        activeProtect = Number(btn.dataset.protect) as ProtectLevel;
        renderProtectTabs();
        renderRiskSections((root.querySelector("#crs-rule-filter") as HTMLInputElement).value);
      };
    });
  }

  function renderRiskSections(filter = ""): void {
    const container = root.querySelector("#crs-tier-rules")!;
    const renderOpts = {
      category: activeCategory,
      protect: activeProtect,
      allRows,
      moduleConfigs,
      filter,
      idPrefix: "risk-",
      pageState,
    };
    container.innerHTML = renderRiskSectionsHtml(renderOpts);
    bindRiskSectionsPagination(container, renderOpts, () =>
      renderRiskSections((root.querySelector("#crs-rule-filter") as HTMLInputElement).value),
    );
  }

  function renderAllRuleViews(filter = ""): void {
    renderCategoryTabs();
    renderProtectTabs();
    renderRiskSections(filter);
  }

  async function rollback(versionId: string): Promise<void> {
    const ok = await showConfirm({
      title: "确认回滚",
      message: `回滚到版本 ${versionId}？\n回滚前会自动保存当前规则集。`,
      okText: "回滚",
      danger: true,
    });
    if (!ok) return;
    try {
      const r = await apiPost<{ rules: number }>("/api/rules/crs/rollback", { version_id: versionId });
      toast(`已回滚，当前 ${r.rules} 条 L7 规则生效`);
      await refresh();
    } catch (e) {
      toast(String(e), false);
    }
  }

  async function refresh(): Promise<void> {
    const [inv, ver, data] = await Promise.all([
      apiGet<RulesInventory & { rules_total: number }>("/api/rules/packs?limit=2000"),
      apiGet<{ versions: CrsVersion[]; max_versions: number }>("/api/rules/crs/versions"),
      fetchRuleRows(),
    ]);

    moduleConfigs = data.moduleConfigs;
    allRows = data.allRows;

    if (!allRows.some((r) => normalizeCategory(r.category) === activeCategory)) {
      activeCategory =
        CATEGORY_ORDER.find((c) => allRows.some((r) => normalizeCategory(r.category) === c)) ?? "sqli";
    }
    const mod = moduleConfigs[activeCategory];
    if (mod && !presetLockedProtect) activeProtect = sensitivityToProtect(mod.sensitivity);

    const crsRules = inv.packs.reduce((n, p) => n + p.rule_count, 0);
    const activeN = allRows.filter((r) => r.active).length;
    const highN = allRows.filter((r) => r.risk === "high").length;
    const medN = allRows.filter((r) => r.risk === "medium").length;
    const lowN = allRows.filter((r) => r.risk === "low").length;

    root.querySelector("#crs-stats")!.innerHTML = `
      <div class="stat-card"><div class="stat-card-value">${activeN}</div><div class="stat-card-label">当前生效规则</div></div>
      <div class="stat-card"><div class="stat-card-value">${highN}</div><div class="stat-card-label">高危规则</div></div>
      <div class="stat-card"><div class="stat-card-value">${medN}</div><div class="stat-card-label">中危规则</div></div>
      <div class="stat-card"><div class="stat-card-value">${lowN}</div><div class="stat-card-label">低危规则</div></div>`;

    cachedPacks = inv.packs;
    cachedCrsRules = crsRules;
    cachedMaxVersions = ver.max_versions;
    cachedVersions = ver.versions;
    paintPacks();
    paintVersions();

    renderAllRuleViews((root.querySelector("#crs-rule-filter") as HTMLInputElement)?.value ?? "");
  }

  root.querySelector("#crs-rule-filter")?.addEventListener("input", (e) => {
    for (const k of Object.keys(pageState)) pageState[k].page = 1;
    renderAllRuleViews((e.target as HTMLInputElement).value);
  });

  root.querySelector("#crs-update")?.addEventListener("click", async () => {
    const ok = await showConfirm({
      title: "更新 CRS 规则集",
      message: "从官方规则仓库导入最新规则集？\n当前规则会先自动备份。",
      okText: "开始更新",
    });
    if (!ok) return;
    const btn = root.querySelector("#crs-update") as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "更新中…";
    try {
      const r = await apiPost<{
        rules: number;
        import: { rules_imported: number; packs_written: number };
        backup_version: { id: string };
      }>("/api/rules/crs/update", { crs_version: "v4.25.0", paranoia_max: 2 });
      toast(
        `更新完成：导入 ${r.import.rules_imported} 条，${r.rules} 条 L7 规则生效（备份 ${r.backup_version.id}）`,
      );
      await refresh();
    } catch (e) {
      toast(String(e), false);
    } finally {
      btn.disabled = false;
      btn.textContent = "更新最新 CRS 规则集";
    }
  });

  root.querySelector("#crs-reload")?.addEventListener("click", async () => {
    try {
      const r = await apiPost<{ rules: number }>("/api/rules/packs", {});
      toast(`已热加载 ${r.rules} 条规则`);
      await refresh();
    } catch (e) {
      toast(String(e), false);
    }
  });

  root.querySelector("#crs-snapshot")?.addEventListener("click", async () => {
    try {
      const r = await apiPost<{ version: CrsVersion }>("/api/rules/crs/snapshot", { note: "手动快照" });
      toast(`已保存版本 ${r.version.id}`);
      await refresh();
    } catch (e) {
      toast(String(e), false);
    }
  });

  root.querySelector("#crs-refresh")?.addEventListener("click", () =>
    void refresh().catch((e) => toast(String(e), false)),
  );

  await refresh();
}
