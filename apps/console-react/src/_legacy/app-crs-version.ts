/** Legacy (pre-React) console module: App crs version. */
import { apiGet, apiPost } from "./api.js";
import {
  btn,
  esc,
  hideModal,
  showConfirm,
  showResult,
  tag,
} from "./components/ui.js";
import type { CrsVersion } from "./crs-rules-shared.js";
import { showModuleRulesModal } from "./module-rules-modal.js";
import { MODULE_META } from "./module-meta.js";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "./crs-preset.js";
import { siteQuery } from "./sites.js";

export type AppCrsBinding = {
  site_id: string;
  crs_version_id: string;
  crs_version_label?: string;
  rule_count: number;
  applied_at: string;
};

export type AppCrsHistoryEntry = {
  id: string;
  site_id: string;
  crs_version_id: string;
  crs_version_label?: string;
  rule_count: number;
  modules_snapshot: Record<
    string,
    { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }
  >;
  is_active: boolean;
  note?: string;
  applied_at: string;
};

export type AppCrsStatus = {
  binding: AppCrsBinding | null;
  system_latest: {
    crs_version_id: string;
    crs_version_label?: string;
    rule_count: number;
  } | null;
  behind_system: boolean;
};

function formatVersionLabel(b: { crs_version_id: string; crs_version_label?: string; rule_count: number }): string {
  const label = b.crs_version_label ?? b.crs_version_id;
  return `${label} · ${b.crs_version_id} · ${b.rule_count} 条`;
}

function el(tagName: string, cls?: string, html?: string): HTMLElement {
  const n = document.createElement(tagName);
  if (cls) n.className = cls;
  if (html != null) n.innerHTML = html;
  return n;
}

export async function fetchAppCrsStatus(siteId: string): Promise<AppCrsStatus> {
  return apiGet<AppCrsStatus>(`/api/apps/${encodeURIComponent(siteId)}/crs`);
}

export function renderAppCrsVersionBar(status: AppCrsStatus, siteId: string): string {
  const cur = status.binding
    ? formatVersionLabel(status.binding)
    : "未绑定（跟随系统当前活跃规则集）";
  const sys = status.system_latest ? formatVersionLabel(status.system_latest) : "—";
  const lagTag = status.behind_system
    ? tag("可升级", "orange")
    : status.binding
      ? tag("已同步", "green")
      : "";
  return `
    <div class="app-crs-version-bar">
      <div class="app-crs-version-row">
        <span><strong>CRS 规则集</strong></span>
        <span class="app-crs-version-actions">
          ${btn({ label: "选择版本", variant: "primary", size: "sm", className: "app-crs-apply-btn", attrs: { "data-site-id": siteId } })}
          ${btn({ label: "历史版本", size: "sm", className: "app-crs-history-btn", attrs: { "data-site-id": siteId } })}
        </span>
      </div>
      <div class="app-crs-version-row">
        <span>当前版本：<code>${esc(cur)}</code> ${lagTag}</span>
      </div>
      <div class="app-crs-version-row">
        <span>系统最新：<code>${esc(sys)}</code></span>
      </div>
    </div>`;
}

function formatCrsTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString("zh-CN");
}

async function openApplyCrsModal(siteId: string, onApplied: () => void): Promise<void> {
  const { versions } = await apiGet<{ versions: CrsVersion[] }>("/api/rules/crs/versions");
  hideModal();
  const mask = el("div", "ant-modal-mask");
  const wrap = el("div", "ant-modal-wrap");
  const modal = el("div", "ant-modal ant-modal-xl");
  modal.innerHTML = `
    <div class="ant-modal-header"><span>选择 CRS 版本</span>
      <button type="button" class="ant-modal-close">×</button></div>
    <div class="ant-modal-body">
      <div class="app-crs-table-wrap">
        <table class="data-table app-crs-ver-table">
          <thead><tr>
            <th class="col-id">版本 ID</th>
            <th class="col-time">时间</th>
            <th class="col-num">规则数</th>
            <th class="col-note">说明</th>
            <th class="col-actions">操作</th>
          </tr></thead>
          <tbody>
          ${versions
            .map(
              (v) => `<tr>
                <td class="col-id"><code>${esc(v.id)}</code></td>
                <td class="col-time muted small">${esc(formatCrsTime(v.created_at))}</td>
                <td class="col-num">${v.rule_count}</td>
                <td class="col-note muted">${esc(v.note || v.crs_version || "—")}</td>
                <td class="col-actions">${btn({ label: "应用", variant: "primary", size: "sm", className: "btn-apply-crs-ver", attrs: { "data-ver": v.id } })}</td>
              </tr>`,
            )
            .join("")}
          </tbody>
        </table>
      </div>
    </div>`;
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  const close = () => mask.remove();
  modal.querySelector(".ant-modal-close")?.addEventListener("click", close);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  modal.querySelectorAll<HTMLButtonElement>(".btn-apply-crs-ver").forEach((b) => {
    b.onclick = () => {
      const ver = b.dataset.ver!;
      void (async () => {
        const confirmed = await showConfirm({
          title: "应用 CRS 版本",
          message: `将本应用 CRS 切换到版本 ${ver}？\n\n系统全局规则集不会被修改；当前配置将自动存入历史。`,
          okText: "应用",
        });
        if (!confirmed) return;
        try {
          await apiPost(`/api/apps/${encodeURIComponent(siteId)}/crs/apply`, {
            crs_version_id: ver,
          });
          await showResult({
            type: "success",
            title: "应用成功",
            message: `已应用 CRS 版本 ${ver}`,
          });
          close();
          onApplied();
        } catch (e) {
          await showResult({
            type: "error",
            title: "应用失败",
            message: e instanceof Error ? e.message : String(e),
          });
        }
      })();
    };
  });
  document.body.appendChild(mask);
}

async function openHistoryModal(siteId: string): Promise<void> {
  const { history } = await apiGet<{ history: AppCrsHistoryEntry[] }>(
    `/api/apps/${encodeURIComponent(siteId)}/crs/history`,
  );
  hideModal();
  const mask = el("div", "ant-modal-mask");
  const wrap = el("div", "ant-modal-wrap");
  const modal = el("div", "ant-modal ant-modal-xl");
  const rows =
    history.length === 0
      ? `<tr><td colspan="5" class="muted">暂无历史记录 — 首次「选择版本并应用」后会自动保存</td></tr>`
      : history
          .map(
            (h) => `<tr class="${h.is_active ? "row-active" : ""}">
              <td class="col-id">${h.is_active ? tag("当前", "green") : ""} <code>${esc(h.crs_version_id)}</code></td>
              <td class="col-time muted small">${esc(formatCrsTime(h.applied_at))}</td>
              <td class="col-num">${h.rule_count}</td>
              <td class="col-note muted">${esc(h.note ?? h.crs_version_label ?? "—")}</td>
              <td class="col-actions">${btn({ label: "查看配置", size: "sm", className: "btn-view-crs-hist", attrs: { "data-id": h.id } })}</td>
            </tr>`,
          )
          .join("");
  modal.innerHTML = `
    <div class="ant-modal-header"><span>CRS 历史版本</span>
      <button type="button" class="ant-modal-close">×</button></div>
    <div class="ant-modal-body">
      <div class="app-crs-table-wrap">
        <table class="data-table app-crs-ver-table">
          <thead><tr>
            <th class="col-id">版本</th>
            <th class="col-time">应用时间</th>
            <th class="col-num">规则数</th>
            <th class="col-note">说明</th>
            <th class="col-actions">操作</th>
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>
    </div>`;
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  const close = () => mask.remove();
  modal.querySelector(".ant-modal-close")?.addEventListener("click", close);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  modal.querySelectorAll<HTMLButtonElement>(".btn-view-crs-hist").forEach((b) => {
    b.onclick = () => {
      const id = b.dataset.id!;
      const entry = history.find((h) => h.id === id);
      if (!entry) return;
      void openHistoryConfigModal(siteId, entry);
    };
  });
  document.body.appendChild(mask);
}

async function openHistoryConfigModal(siteId: string, entry: AppCrsHistoryEntry): Promise<void> {
  hideModal();
  const mask = el("div", "ant-modal-mask");
  const wrap = el("div", "ant-modal-wrap");
  const modal = el("div", "ant-modal ant-modal-lg");
  const moduleRows = Object.entries(entry.modules_snapshot)
    .map(([key, cfg]) => {
      const meta = MODULE_META[key] ?? { title: key, desc: "" };
      const disabled = cfg.disabled_rules?.length ?? 0;
      const crsCat = MODULE_CRS_CATEGORY[key];
      return `<tr>
        <td><strong>${esc(meta.title)}</strong><div class="muted small">${esc(key)}</div></td>
        <td>${cfg.enabled ? tag("开", "green") : tag("关")}</td>
        <td>${esc(cfg.action)} / ${esc(cfg.sensitivity)}</td>
        <td>${disabled ? `<button type="button" class="ant-btn ant-btn-link ant-btn-sm btn-hist-disabled" data-key="${esc(key)}">已禁用 ${disabled} 条</button>` : "—"}</td>
        <td>${crsCat ? btn({ label: "查看规则", size: "sm", className: "btn-hist-rules", attrs: { "data-key": key, "data-cat": crsCat } }) : ""}</td>
      </tr>`;
    })
    .join("");
  modal.innerHTML = `
    <div class="ant-modal-header"><span>历史配置 · ${esc(entry.crs_version_id)}</span>
      <button type="button" class="ant-modal-close">×</button></div>
    <div class="ant-modal-body">
      <p class="muted small">只读快照 · 应用时间 ${esc(entry.applied_at)}${entry.note ? ` · ${esc(entry.note)}` : ""}</p>
      <table class="data-table">
        <thead><tr><th>模块</th><th>开关</th><th>动作/灵敏度</th><th>禁用规则</th><th></th></tr></thead>
        <tbody>${moduleRows || `<tr><td colspan="5" class="muted">无模块快照</td></tr>`}</tbody>
      </table>
    </div>`;
  wrap.appendChild(modal);
  mask.appendChild(wrap);
  const close = () => mask.remove();
  modal.querySelector(".ant-modal-close")?.addEventListener("click", close);
  mask.addEventListener("click", (e) => {
    if (e.target === mask) close();
  });
  modal.querySelectorAll<HTMLButtonElement>(".btn-hist-disabled").forEach((b) => {
    b.onclick = () => {
      const key = b.dataset.key!;
      const cfg = entry.modules_snapshot[key];
      if (!cfg?.disabled_rules?.length) return;
      const cat = MODULE_CRS_CATEGORY[key];
      if (!cat) return;
      void showModuleRulesModal({
        category: cat,
        protect: sensitivityToProtectLevel(cfg.sensitivity),
        moduleTitle: (MODULE_META[key]?.title ?? key) + "（历史 · 已禁用）",
        moduleEnabled: cfg.enabled,
        moduleKey: key,
        siteId,
        editable: false,
        disabledRules: cfg.disabled_rules,
      });
    };
  });
  modal.querySelectorAll<HTMLButtonElement>(".btn-hist-rules").forEach((b) => {
    b.onclick = () => {
      const key = b.dataset.key!;
      const cat = b.dataset.cat!;
      const cfg = entry.modules_snapshot[key];
      if (!cfg) return;
      void showModuleRulesModal({
        category: cat,
        protect: sensitivityToProtectLevel(cfg.sensitivity),
        moduleTitle: (MODULE_META[key]?.title ?? key) + "（历史）",
        moduleEnabled: cfg.enabled,
        moduleKey: key,
        siteId,
        editable: false,
        disabledRules: cfg.disabled_rules ?? [],
      });
    };
  });
  document.body.appendChild(mask);
}

export function bindAppCrsVersionBar(root: HTMLElement, onApplied: () => void): void {
  root.querySelectorAll<HTMLButtonElement>(".app-crs-apply-btn").forEach((btnEl) => {
    btnEl.addEventListener("click", () => {
      const siteId = btnEl.dataset.siteId;
      if (!siteId) return;
      void openApplyCrsModal(siteId, onApplied);
    });
  });
  root.querySelectorAll<HTMLButtonElement>(".app-crs-history-btn").forEach((btnEl) => {
    btnEl.addEventListener("click", () => {
      const siteId = btnEl.dataset.siteId;
      if (!siteId) return;
      void openHistoryModal(siteId);
    });
  });
}
