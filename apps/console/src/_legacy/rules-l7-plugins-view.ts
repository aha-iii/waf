/** Legacy L7 JSON/Lua plugins view with ConfigMap-style versioning. */
import { apiDelete, apiGet, apiPost, apiPut } from "./api.js";
import {
  btn,
  el,
  emptyState,
  esc,
  formField,
  hideModal,
  mountNerveDialog,
  nerveForm,
  nerveInput,
  nerveTag,
  pageHeader,
  runMutatingAction,
  showConfirm,
  showResult,
  syncActions,
  textarea,
} from "./components/ui.js";
import { PAGE_META, type PageId } from "./nav.js";

type PluginType = "json" | "lua";

type L7PluginSummary = {
  name: string;
  version: string;
  description: string;
  type: string;
  category?: string;
  priority: number;
  enabled: boolean;
  updatedAt?: string;
  updated_at?: string;
};

type PluginDetail = {
  type: string;
  manifest: {
    name: string;
    version: string;
    description: string;
    type: string;
    category?: string;
    priority: number;
    enabled: boolean;
  };
  rulesJson?: string;
  script?: string;
  updatedAt?: string;
};

type VersionRow = {
  version: string;
  publishedAt?: string;
  published_at?: string;
  note?: string;
  current?: boolean;
};

function updatedAtOf(p: L7PluginSummary): string {
  return p.updatedAt ?? p.updated_at ?? "";
}

function fmtTime(raw?: string): string {
  if (!raw) return "—";
  const n = Number(raw);
  if (!Number.isNaN(n) && n > 1_000_000_000) {
    return new Date(n * 1000).toLocaleString("zh-CN");
  }
  const d = new Date(raw);
  if (!Number.isNaN(d.getTime())) return d.toLocaleString("zh-CN");
  return raw;
}

function verLabel(v: string): string {
  const s = (v || "").trim();
  if (!s) return "—";
  return s.startsWith("v") ? s : `v${s}`;
}

export async function renderRulesL7PluginsView(
  root: HTMLElement,
  toast: (msg: string, ok?: boolean) => void,
  pluginType: PluginType,
): Promise<void> {
  const pageId = (pluginType === "json" ? "rules-plugins-json" : "rules-plugins-lua") as PageId;
  const meta = PAGE_META[pageId];
  root.innerHTML = `${pageHeader(
    meta.title,
    meta.subtitle,
    syncActions([
      btn({ id: "l7p-new", label: "新建", size: "sm", variant: "primary" }),
      btn({ id: "l7p-refresh", label: "刷新", size: "sm" }),
    ]),
  )}<div id="l7p-body" class="page-pad">${emptyState("加载中…")}</div>`;

  const body = root.querySelector("#l7p-body") as HTMLElement;
  let docBound = false;

  function closeAllMoreMenus(except?: HTMLElement): void {
    body.querySelectorAll<HTMLElement>(".l7p-more.open").forEach((wrap) => {
      if (except && wrap === except) return;
      wrap.classList.remove("open");
      const menu = wrap.querySelector<HTMLElement>(".l7p-more-menu");
      if (menu) menu.hidden = true;
    });
  }

  async function load(): Promise<void> {
    try {
      const data = await apiGet<{ plugins: L7PluginSummary[] }>(
        `/api/plugins/l7?type=${pluginType}`,
      );
      const list = data.plugins ?? [];
      if (!list.length) {
        body.innerHTML = emptyState(`暂无 ${pluginType.toUpperCase()} 插件`);
        return;
      }
      body.innerHTML = `<div class="l7p-list">${list
        .map((p) => {
          const cat = p.category ? nerveTag(p.category, "blue") : "";
          const st = nerveTag(p.enabled ? "启用" : "禁用", p.enabled ? "green" : "default");
          return `<div class="l7p-row" data-name="${esc(p.name)}">
            <div class="l7p-row-main">
              <button type="button" class="l7p-name ant-btn-link" data-act="view">${esc(p.name)}</button>
              <span class="l7p-ver-badge">${nerveTag("当前版本", "blue")} <code>${esc(verLabel(p.version))}</code></span>
              <span class="l7p-meta">${cat}${st}</span>
            </div>
            <div class="l7p-row-time muted small">${esc(fmtTime(updatedAtOf(p)))}</div>
            <div class="l7p-row-ops">
              <button type="button" class="ant-btn ant-btn-link ant-btn-sm" data-act="publish">更新</button>
              <div class="l7p-more">
                <button type="button" class="ant-btn ant-btn-link ant-btn-sm" data-act="more">更多 ▾</button>
                <div class="l7p-more-menu" hidden>
                  <button type="button" data-act="history">历史版本</button>
                  <button type="button" data-act="toggle">${p.enabled ? "禁用" : "启用"}</button>
                  <button type="button" class="danger" data-act="delete">删除</button>
                </div>
              </div>
            </div>
          </div>`;
        })
        .join("")}</div>`;
      bindRows(list);
    } catch (e) {
      body.innerHTML = `<div class="ant-alert ant-alert-error">${esc(e instanceof Error ? e.message : String(e))}</div>`;
    }
  }

  function bindRows(list: L7PluginSummary[]): void {
    body.querySelectorAll<HTMLElement>(".l7p-row").forEach((row) => {
      const name = row.dataset.name;
      if (!name) return;
      const item = list.find((p) => p.name === name);
      row.querySelectorAll<HTMLElement>("[data-act]").forEach((btnEl) => {
        btnEl.onclick = (ev) => {
          ev.preventDefault();
          ev.stopPropagation();
          const act = btnEl.dataset.act;
          if (act === "more") {
            const wrap = btnEl.closest<HTMLElement>(".l7p-more");
            if (!wrap) return;
            const willOpen = !wrap.classList.contains("open");
            closeAllMoreMenus(wrap);
            wrap.classList.toggle("open", willOpen);
            const menu = wrap.querySelector<HTMLElement>(".l7p-more-menu");
            if (menu) menu.hidden = !willOpen;
            return;
          }
          closeAllMoreMenus();
          if (act === "view") void openView(name);
          else if (act === "publish") void openPublish(name);
          else if (act === "history") void openHistory(name);
          else if (act === "toggle" && item) {
            const enabled = !item.enabled;
            void runMutatingAction({
              action: "edit",
              subject: name,
              confirmMessage: `确认${enabled ? "启用" : "禁用"}插件「${name}」？`,
              run: async () => {
                await apiPut(`/api/plugins/l7/${encodeURIComponent(name)}`, { enabled });
                await load();
              },
            });
          } else if (act === "delete") {
            void runMutatingAction({
              action: "delete",
              subject: name,
              confirmMessage: `确认删除插件「${name}」？`,
              run: async () => {
                await apiDelete(`/api/plugins/l7/${encodeURIComponent(name)}`);
                await load();
              },
            });
          }
        };
      });
    });
    if (!docBound) {
      docBound = true;
      document.addEventListener("click", () => closeAllMoreMenus());
    }
  }

  function contentField(detail: PluginDetail | null, readonly: boolean): string {
    if (pluginType === "json") {
      return formField(
        "规则 JSON",
        textarea({
          id: "l7p-rules",
          rows: 16,
          className: "mono",
          value:
            detail?.rulesJson ??
            '[\n  {"id":"rule-001","name":"sample","pattern":"(?i)attack","category":"custom","severity":"medium"}\n]',
          attrs: readonly ? { readonly: true } : undefined,
        }),
      );
    }
    return formField(
      "Lua 脚本",
      textarea({
        id: "l7p-script",
        rows: 16,
        className: "mono",
        value: detail?.script ?? `function inspect(req)\n  return nil\nend\n`,
        attrs: readonly ? { readonly: true } : undefined,
      }),
    );
  }

  async function openView(name: string): Promise<void> {
    const detail = await apiGet<PluginDetail>(`/api/plugins/l7/${encodeURIComponent(name)}`);
    const m = detail.manifest;
    hideModal();
    const { body: modalBody, footer, close } = mountNerveDialog({
      title: `查看 · ${name}`,
      wide: true,
    });
    modalBody.innerHTML = nerveForm(
      [
        formField("版本", `<code>${esc(verLabel(m.version))}</code>`),
        formField(
          "说明",
          nerveInput({ id: "l7p-desc", value: m.description ?? "", wide: true, attrs: { disabled: true } }),
        ),
        formField(
          "分类",
          nerveInput({ id: "l7p-cat", value: m.category ?? "", attrs: { disabled: true } }),
        ),
        formField(
          "优先级",
          nerveInput({ id: "l7p-pri", value: String(m.priority ?? ""), attrs: { disabled: true } }),
        ),
        contentField(detail, true),
      ].join(""),
    );
    const closeBtn = el("button", "nerve-btn ant-btn", "关闭");
    closeBtn.type = "button";
    closeBtn.onclick = close;
    footer.append(closeBtn);
  }

  async function openPublish(name: string | null): Promise<void> {
    const isNew = !name;
    let detail: PluginDetail | null = null;
    if (!isNew && name) {
      detail = await apiGet<PluginDetail>(`/api/plugins/l7/${encodeURIComponent(name)}`);
    }
    const title = isNew
      ? `新建 ${pluginType === "json" ? "JSON" : "Lua"} 插件`
      : `更新 · ${name}`;
    hideModal();
    const { body: modalBody, footer, close } = mountNerveDialog({ title, wide: true });
    const m = detail?.manifest;
    const fields = [
      formField(
        "名称",
        nerveInput({
          id: "l7p-name",
          value: m?.name ?? "",
          placeholder: "my-plugin",
          attrs: isNew ? undefined : { disabled: true },
        }),
      ),
      ...(isNew
        ? []
        : [formField("当前版本", `<code>${esc(verLabel(m?.version ?? ""))}</code>`)]),
      formField("说明", nerveInput({ id: "l7p-desc", value: m?.description ?? "", wide: true })),
      formField(
        "分类",
        nerveInput({
          id: "l7p-cat",
          value: m?.category ?? "custom",
          placeholder: "xss / sqli / custom",
        }),
      ),
      formField(
        "优先级",
        nerveInput({ id: "l7p-pri", type: "number", value: String(m?.priority ?? 100) }),
      ),
      ...(isNew
        ? []
        : [
            formField(
              "发布说明",
              nerveInput({ id: "l7p-note", value: "", wide: true, placeholder: "可选" }),
            ),
          ]),
      contentField(detail, false),
    ];
    modalBody.innerHTML = nerveForm(fields.join(""));

    const cancel = el("button", "nerve-btn ant-btn", "取消");
    cancel.type = "button";
    cancel.onclick = close;
    const save = el("button", "nerve-btn ant-btn ant-btn-primary", isNew ? "创建" : "发布");
    save.type = "button";
    save.onclick = () => {
      void (async () => {
        const q = (id: string) =>
          modalBody.querySelector(`#${id}`) as HTMLInputElement | HTMLTextAreaElement | null;
        try {
          if (isNew) {
            const newName = (q("l7p-name")?.value ?? "").trim();
            const payload: Record<string, unknown> = {
              name: newName,
              type: pluginType,
              description: q("l7p-desc")?.value ?? "",
              category: q("l7p-cat")?.value || "custom",
              priority: Number(q("l7p-pri")?.value || 100),
              enabled: true,
            };
            if (pluginType === "json") payload.rules_json = q("l7p-rules")?.value ?? "[]";
            else payload.script = q("l7p-script")?.value ?? "";
            await apiPost("/api/plugins/l7", payload);
            close();
            await showResult({ type: "success", title: "已创建", message: `${newName} · v1.0.0` });
          } else {
            const payload: Record<string, unknown> = {
              description: q("l7p-desc")?.value ?? "",
              category: q("l7p-cat")?.value || "custom",
              priority: Number(q("l7p-pri")?.value || 100),
              note: q("l7p-note")?.value ?? "",
            };
            if (pluginType === "json") payload.rules_json = q("l7p-rules")?.value ?? "[]";
            else payload.script = q("l7p-script")?.value ?? "";
            const res = await apiPost<{ version?: string }>(
              `/api/plugins/l7/${encodeURIComponent(name!)}/publish`,
              payload,
            );
            close();
            await showResult({
              type: "success",
              title: "已发布",
              message: `${name} · ${verLabel(res.version ?? "")}`,
            });
          }
          await load();
        } catch (e) {
          await showResult({
            type: "error",
            title: isNew ? "创建失败" : "发布失败",
            message: e instanceof Error ? e.message : String(e),
          });
        }
      })();
    };
    footer.append(cancel, save);
  }

  async function openHistory(name: string): Promise<void> {
    const data = await apiGet<{ current: string; versions: VersionRow[] }>(
      `/api/plugins/l7/${encodeURIComponent(name)}/versions`,
    );
    hideModal();
    const { body: modalBody, footer, close } = mountNerveDialog({
      title: `历史版本 · ${name}`,
      wide: true,
    });
    const rows = (data.versions ?? [])
      .map((v) => {
        const ts = v.publishedAt ?? v.published_at ?? "";
        const isCur = v.current === true || v.version === data.current;
        return `<tr data-ver="${esc(v.version)}">
          <td><code>${esc(verLabel(v.version))}</code>${isCur ? ` ${nerveTag("当前", "blue")}` : ""}</td>
          <td class="muted small">${esc(fmtTime(ts))}</td>
          <td>${esc(v.note || "—")}</td>
          <td class="nerve-col-actions">
            <button type="button" class="ant-btn ant-btn-link ant-btn-sm" data-hist="view">查看</button>
            ${
              isCur
                ? ""
                : `<button type="button" class="ant-btn ant-btn-link ant-btn-sm" data-hist="rollback">回滚到该版本</button>`
            }
          </td>
        </tr>`;
      })
      .join("");
    modalBody.innerHTML = rows
      ? `<table class="data-table nerve-table"><thead><tr>
          <th>版本</th><th>时间</th><th>说明</th><th>操作</th>
        </tr></thead><tbody>${rows}</tbody></table>`
      : emptyState("暂无历史版本");

    modalBody.querySelectorAll<HTMLElement>("[data-hist]").forEach((btnEl) => {
      btnEl.onclick = () => {
        const tr = btnEl.closest("tr");
        const ver = tr?.dataset.ver;
        if (!ver) return;
        if (btnEl.dataset.hist === "view") void openVersionView(name, ver);
        else if (btnEl.dataset.hist === "rollback") {
          void (async () => {
            const ok = await showConfirm({
              title: "回滚确认",
              message: `将以 ${verLabel(ver)} 的内容发布为新版本，是否继续？`,
            });
            if (!ok) return;
            try {
              const res = await apiPost<{ version?: string }>(
                `/api/plugins/l7/${encodeURIComponent(name)}/rollback`,
                { version: ver },
              );
              close();
              await showResult({
                type: "success",
                title: "已回滚发布",
                message: `${name} · ${verLabel(res.version ?? "")}`,
              });
              await load();
            } catch (e) {
              await showResult({
                type: "error",
                title: "回滚失败",
                message: e instanceof Error ? e.message : String(e),
              });
            }
          })();
        }
      };
    });

    const closeBtn = el("button", "nerve-btn ant-btn", "关闭");
    closeBtn.type = "button";
    closeBtn.onclick = close;
    footer.append(closeBtn);
  }

  async function openVersionView(name: string, version: string): Promise<void> {
    const detail = await apiGet<PluginDetail>(
      `/api/plugins/l7/${encodeURIComponent(name)}/versions/${encodeURIComponent(version)}`,
    );
    const m = detail.manifest;
    hideModal();
    const { body: modalBody, footer, close } = mountNerveDialog({
      title: `查看 · ${name} · ${verLabel(version)}`,
      wide: true,
    });
    modalBody.innerHTML = nerveForm(
      [
        formField("版本", `<code>${esc(verLabel(version))}</code>`),
        formField(
          "说明",
          nerveInput({ value: m.description ?? "", wide: true, attrs: { disabled: true } }),
        ),
        formField(
          "分类",
          nerveInput({ value: m.category ?? "", attrs: { disabled: true } }),
        ),
        contentField(detail, true),
      ].join(""),
    );
    const closeBtn = el("button", "nerve-btn ant-btn", "关闭");
    closeBtn.type = "button";
    closeBtn.onclick = close;
    footer.append(closeBtn);
  }

  root.querySelector("#l7p-refresh")?.addEventListener("click", () => void load());
  root.querySelector("#l7p-new")?.addEventListener("click", () => void openPublish(null));

  await load();
}
