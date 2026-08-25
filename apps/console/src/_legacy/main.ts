/** Legacy console module: Main. */
import { apiGet } from "./api.js";
import { ensureAuth, getAuthSkippedReason, getUsername, logout } from "./auth.js";

import { CRS_NAV_EVENT } from "./crs-preset.js";
import { EVENTS_NAV_EVENT } from "./sites.js";
import {
  NAV,
  PAGE_META,
  GATEWAY_DASHBOARD_URL,
  GATEWAY_DASHBOARD_LABEL,
  ALERT_CONSOLE_URL,
  PLATFORM_CONSOLE_URL,
  CONSOLE_BUILD,
  RULES_NAV,
  RULES_NAV_GROUP,
  AI_NAV,
  AI_NAV_GROUP,
  type PageId,
} from "./nav.js";

import { renderAdvancedSecurityView } from "./advanced-security-view.js";
import { renderRulesCrsView } from "./rules-crs-view.js";
import { renderRulesAiView } from "./rules-ai-view.js";
import { renderRulesCustomView } from "./rules-custom-view.js";
import { renderRulesL7PluginsView } from "./rules-l7-plugins-view.js";

import { renderAiSemanticView } from "./ai-semantic-view.js";
import { renderAiLlmView } from "./ai-llm-view.js";
import { renderAiLabView } from "./ai-lab-view.js";

import { renderAccessView } from "./access-view.js";

import { renderAlertsView } from "./alerts-view.js";

import { renderAssetsView } from "./assets-view.js";

import { renderBotView } from "./bot-view.js";

import { renderCcView } from "./cc-view.js";

import { renderCloudDashboard } from "./dashboard-view.js";

import { renderGeoView } from "./geo-view.js";

import { renderIntelView } from "./intel-view.js";

import { renderProtectionLevelsView } from "./protection-levels-view.js";
import { renderProtectionModules, renderSecurityEvents } from "./security-view.js";

import { renderWafAnalyticsView } from "./waf-analytics-view.js";



function el(tag: string, cls?: string, html?: string): HTMLElement {

  const n = document.createElement(tag);

  if (cls) n.className = cls;

  if (html != null) n.innerHTML = html;

  return n;

}



function toast(msg: string, ok = true): void {

  const t = el("div", `toast show toast-${ok ? "ok" : "err"}`, msg);

  document.getElementById("toast-root")?.appendChild(t);

  setTimeout(() => t.remove(), 3000);

}



let page: PageId = "dashboard";

let contentEl: HTMLElement | null = null;

let topbarMetaEl: HTMLElement | null = null;

let topbarHealthEl: HTMLElement | null = null;

let menuEl: HTMLElement | null = null;



function createMenuButton(item: { id: PageId; label: string; icon: string }): HTMLButtonElement {
  const btn = el("button", "menu-item");
  btn.type = "button";
  btn.dataset.page = item.id;
  btn.innerHTML = `<span class="menu-icon">${item.icon}</span><span class="menu-label">${item.label}</span>`;
  btn.onclick = () => navigateTo(item.id);
  return btn;
}

function buildSidebarMenu(menu: HTMLElement): void {
  menu.innerHTML = "";
  let lastGroup = "";
  const seen = new Set<PageId>();

  for (const item of NAV) {
    if (seen.has(item.id)) continue;
    seen.add(item.id);
    if (item.group !== lastGroup) {
      menu.appendChild(el("div", "menu-group-title", item.group));
      lastGroup = item.group;
    }
    menu.appendChild(createMenuButton(item));
  }

  if (!menu.querySelector('[data-page="rules-crs"]')) {
    menu.appendChild(el("div", "menu-group-title", RULES_NAV_GROUP));
    for (const item of RULES_NAV) {
      menu.appendChild(createMenuButton(item));
    }
  }

  if (!menu.querySelector('[data-page="ai-llm"]')) {
    menu.appendChild(el("div", "menu-group-title", AI_NAV_GROUP));
    for (const item of AI_NAV) {
      menu.appendChild(createMenuButton(item));
    }
  }
}

function parseHashPage(): PageId | null {
  const h = location.hash.replace(/^#\/?/, "");
  if (!h) return null;
  const allowed: PageId[] = [
    "rules-crs", "rules-ai", "rules-custom",
    "rules-plugins-lua",
    "ai-semantic", "ai-llm", "ai-lab",
    "dashboard", "advanced", "modules", "protection-levels", "events",
  ];
  // Legacy bookmark #/rules-plugins-json → signature rules
  if (h === "rules-plugins-json") return "rules-custom";
  return allowed.includes(h as PageId) ? (h as PageId) : null;
}

function navigateTo(next: PageId): void {
  page = next;
  location.hash = `#/${next}`;

  menuEl?.querySelectorAll(".menu-item").forEach((b) =>
    b.classList.toggle("active", (b as HTMLElement).dataset.page === page),
  );

  if (contentEl) void renderPage(contentEl);
}



async function refreshTopbarHealth(): Promise<void> {

  if (!topbarHealthEl) return;

  try {

    const [health, nerve] = await Promise.all([

      apiGet<{ ok: boolean; eventBackend: string; databaseConfigured: boolean }>("/api/health"),

      apiGet<{ status?: { ok?: boolean } }>("/api/nerve/status").catch(() => null),

    ]);

    const now = new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });

    topbarHealthEl.innerHTML = `

      <a class="ant-btn ant-btn-sm" href="${GATEWAY_DASHBOARD_URL}" target="_blank" rel="noopener">${GATEWAY_DASHBOARD_LABEL}</a>

      <a class="ant-btn ant-btn-sm" href="${ALERT_CONSOLE_URL}" target="_blank" rel="noopener">Nerve-alert</a>

      <span class="health-pill ${health.ok ? "ok" : "err"}"><i class="health-dot"></i>API</span>

      <span class="health-pill ${nerve ? "ok" : "warn"}"><i class="health-dot"></i>Agent</span>

      <span class="topbar-time muted">${now}</span>

      <button type="button" class="ant-btn ant-btn-sm" id="btn-top-refresh">↻</button>`;

    topbarHealthEl.querySelector("#btn-top-refresh")?.addEventListener("click", () => {

      if (contentEl) void renderPage(contentEl);

    });

  } catch {

    topbarHealthEl.innerHTML = `<span class="health-pill err">API 离线</span>`;

  }

}



function updateTopbarBreadcrumb(): void {

  if (!topbarMetaEl) return;

  const item = NAV.find((n) => n.id === page)!;

  topbarMetaEl.innerHTML = `

    <span class="bc-muted">Nerve WAF</span><span class="bc-sep">/</span>

    <span class="bc-muted">${item.group}</span><span class="bc-sep">/</span>

    <span class="bc-current">${item.label}</span>`;

}



async function renderPage(root: HTMLElement): Promise<void> {

  updateTopbarBreadcrumb();

  root.innerHTML = `<div class="page-loading">加载中…</div>`;

  try {

    switch (page) {

      case "dashboard":

        await renderCloudDashboard(root, (p) => navigateTo(p as PageId));

        break;

      case "assets":

        await renderAssetsView(root, toast);

        break;

      case "events":

        await renderSecurityEvents(root, toast);

        break;

      case "modules":

        await renderProtectionModules(root, toast);

        break;

      case "protection-levels":

        await renderProtectionLevelsView(root, toast);

        break;

      case "cc":

        await renderCcView(root, toast);

        break;

      case "geo":

        await renderGeoView(root, toast);

        break;

      case "bot":

        await renderBotView(root, toast);

        break;

      case "intel":

        await renderIntelView(root, toast);

        break;

      case "access":

        await renderAccessView(root, toast);

        break;

      case "analytics":

        await renderWafAnalyticsView(root, toast);

        break;

      case "alerts":

        await renderAlertsView(root, toast);

        break;

      case "advanced":
        await renderAdvancedSecurityView(root, toast, navigateTo);
        break;

      case "rules-crs":

        await renderRulesCrsView(root, toast);

        break;

      case "rules-ai":

        await renderRulesAiView(root, toast);

        break;

      case "rules-custom":
        await renderRulesCustomView(root, toast);
        break;

      case "rules-plugins-lua":
        await renderRulesL7PluginsView(root, toast, "lua");
        break;

      case "ai-semantic":
        await renderAiSemanticView(root, toast);
        break;

      case "ai-llm":
        await renderAiLlmView(root, toast);
        break;

      case "ai-lab":
        await renderAiLabView(root, toast);
        break;

    }

    root.classList.add("page-inner");

    void refreshTopbarHealth();

  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    const hint =
      page === "alerts"
        ? "告警服务暂不可用，请稍后重试"
        : "WAF API 暂不可用，请稍后重试";
    root.innerHTML = `<div class="ant-alert ant-alert-error">${msg}<br><small>${hint}</small></div>`;
  }

}



export function buildApp(root: HTMLElement): void {

  document.body.appendChild(Object.assign(el("div"), { id: "toast-root" }));



  const layout = el("div", "console-layout");

  const sider = el("aside", "console-sider");

  sider.innerHTML = `

    <div class="sider-logo">

      <span class="logo-icon">N</span>

      <div><strong>Nerve WAF</strong><small>安全检测 Console</small></div>

    </div>

    <div class="sider-search"><input type="text" class="sider-search-input" placeholder="搜索 WAF 功能…" id="menu-search" /></div>`;



  const menu = el("nav", "sider-menu");
  menuEl = menu;
  buildSidebarMenu(menu);
  sider.appendChild(menu);

  const siderFoot = el("div", "sider-foot muted small", `Console ${CONSOLE_BUILD}`);
  sider.appendChild(siderFoot);



  const main = el("div", "console-main");

  const topbar = el("header", "console-topbar");

  topbar.innerHTML = `

    <div class="breadcrumb" id="topbar-bc"></div>

    <div class="topbar-center muted small">WAF 安全 · <a href="${GATEWAY_DASHBOARD_URL}" target="_blank" rel="noopener">${GATEWAY_DASHBOARD_LABEL} →</a> · <a href="${ALERT_CONSOLE_URL}" target="_blank" rel="noopener">Nerve-alert →</a></div>

    <div class="topbar-right" id="topbar-health"></div>
    <div class="topbar-user muted small" id="topbar-user" hidden></div>`;

  topbarMetaEl = topbar.querySelector("#topbar-bc");

  topbarHealthEl = topbar.querySelector("#topbar-health");

  const userEl = topbar.querySelector("#topbar-user") as HTMLElement | null;
  const uname = getUsername();
  if (userEl && uname) {
    userEl.hidden = false;
    userEl.innerHTML = `${uname} · <a href="#" id="logout-link">退出</a>`;
    userEl.querySelector("#logout-link")?.addEventListener("click", (ev) => {
      ev.preventDefault();
      void logout();
    });
  }

  const content = el("main", "console-content");

  contentEl = content;

  layout.append(sider, main);

  main.append(topbar, content, el("footer", "app-copyright", "Copyright (c) 2026 Nerve WAF. All rights reserved."));

  root.appendChild(layout);



  sider.querySelector("#menu-search")?.addEventListener("input", (ev) => {
    const q = (ev.target as HTMLInputElement).value.trim().toLowerCase();
    menu.querySelectorAll<HTMLElement>(".menu-item").forEach((btn) => {
      const id = btn.dataset.page as PageId;
      const item = NAV.find((n) => n.id === id) ?? RULES_NAV.find((n) => n.id === id) ?? AI_NAV.find((n) => n.id === id);
      const hay = `${item?.label ?? ""} ${item?.group ?? ""}`.toLowerCase();
      btn.style.display = !q || hay.includes(q) ? "" : "none";
    });
  });

  const hashPage = parseHashPage();
  if (hashPage) page = hashPage;
  window.addEventListener("hashchange", () => {
    const p = parseHashPage();
    if (p) navigateTo(p);
  });
  window.addEventListener(CRS_NAV_EVENT, () => {
    page = "rules-crs";
    menuEl?.querySelectorAll(".menu-item").forEach((b) =>
      b.classList.toggle("active", (b as HTMLElement).dataset.page === "rules-crs"),
    );
    if (contentEl) void renderPage(contentEl);
  });
  window.addEventListener(EVENTS_NAV_EVENT, () => {
    page = "events";
    menuEl?.querySelectorAll(".menu-item").forEach((b) =>
      b.classList.toggle("active", (b as HTMLElement).dataset.page === "events"),
    );
    if (contentEl) void renderPage(contentEl);
  });

  menu.querySelector(`[data-page="${page}"]`)?.classList.add("active");
  void renderPage(content);
}



const appRoot = document.getElementById("app");

async function bootstrap(): Promise<void> {
  await ensureAuth();
  if (!appRoot) return;
  buildApp(appRoot);
  const skipReason = getAuthSkippedReason();
  if (skipReason) {
    const banner = document.createElement("div");
    banner.className = "ant-alert ant-alert-warning";
    banner.style.cssText = "margin:12px 16px 0;border-radius:6px";
    banner.textContent = skipReason;
    const main = appRoot.querySelector(".console-main");
    main?.insertBefore(banner, main.firstChild);
  }
}

if (appRoot) {
  bootstrap().catch((e) => {
    const msg = e instanceof Error ? e.message : String(e);
    appRoot.innerHTML = `<div class="ant-alert ant-alert-error">登录失败：${msg}</div>`;
  });
}

