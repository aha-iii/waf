import { useCallback, useEffect, useState } from "react";
import {
  NERVE_COL_ACTIONS,
  NerveAlert,
  NerveButton,
  NerveEmpty,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NervePageHeader,
  NerveSyncActions,
  NerveTable,
  NerveTableActions,
  NerveTag,
  NerveTextarea,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiDelete, apiGet, apiPost, apiPut } from "../api";
import { useToast } from "../hooks/useToast";
import { PAGE_META } from "../nav";

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

type PublishForm = {
  name: string;
  description: string;
  category: string;
  priority: number;
  note: string;
  script: string;
};

const DEFAULT_SCRIPT = `function inspect(req)\n  return nil\nend\n`;

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

export function RulesLuaPage() {
  const meta = PAGE_META["rules-plugins-lua"];
  const toast = useToast();
  const [plugins, setPlugins] = useState<L7PluginSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [moreOpen, setMoreOpen] = useState<string | null>(null);

  const [viewModal, setViewModal] = useState<{
    title: string;
    detail: PluginDetail;
    version?: string;
  } | null>(null);

  const [publishModal, setPublishModal] = useState<{ isNew: boolean; name: string | null } | null>(
    null,
  );
  const [publishForm, setPublishForm] = useState<PublishForm>({
    name: "",
    description: "",
    category: "custom",
    priority: 100,
    note: "",
    script: DEFAULT_SCRIPT,
  });
  const [savingPublish, setSavingPublish] = useState(false);

  const [historyModal, setHistoryModal] = useState<{
    name: string;
    current: string;
    versions: VersionRow[];
  } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await apiGet<{ plugins: L7PluginSummary[] }>("/api/plugins/l7?type=lua");
      setPlugins(data.plugins ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!moreOpen) return;
    const onDoc = () => setMoreOpen(null);
    document.addEventListener("click", onDoc);
    return () => document.removeEventListener("click", onDoc);
  }, [moreOpen]);

  const openView = async (name: string, version?: string) => {
    try {
      const detail = version
        ? await apiGet<PluginDetail>(
            `/api/plugins/l7/${encodeURIComponent(name)}/versions/${encodeURIComponent(version)}`,
          )
        : await apiGet<PluginDetail>(`/api/plugins/l7/${encodeURIComponent(name)}`);
      setViewModal({
        title: version ? `查看 · ${name} · ${verLabel(version)}` : `查看 · ${name}`,
        detail,
        version,
      });
    } catch (e) {
      toast(String(e), false);
    }
  };

  const openPublish = async (name: string | null) => {
    const isNew = !name;
    let detail: PluginDetail | null = null;
    if (!isNew && name) {
      try {
        detail = await apiGet<PluginDetail>(`/api/plugins/l7/${encodeURIComponent(name)}`);
      } catch (e) {
        toast(String(e), false);
        return;
      }
    }
    const m = detail?.manifest;
    setPublishForm({
      name: m?.name ?? "",
      description: m?.description ?? "",
      category: m?.category ?? "custom",
      priority: m?.priority ?? 100,
      note: "",
      script: detail?.script ?? DEFAULT_SCRIPT,
    });
    setPublishModal({ isNew, name });
  };

  const submitPublish = async () => {
    if (!publishModal) return;
    const { isNew, name } = publishModal;
    setSavingPublish(true);
    try {
      if (isNew) {
        const newName = publishForm.name.trim();
        if (!newName) {
          await nerveResult({ type: "info", title: "校验失败", message: "请填写名称" });
          return;
        }
        await apiPost("/api/plugins/l7", {
          name: newName,
          type: "lua",
          description: publishForm.description,
          category: publishForm.category || "custom",
          priority: publishForm.priority || 100,
          enabled: true,
          script: publishForm.script,
        });
        setPublishModal(null);
        await nerveResult({ type: "success", title: "已创建", message: `${newName} · v1.0.0` });
      } else {
        const res = await apiPost<{ version?: string }>(
          `/api/plugins/l7/${encodeURIComponent(name!)}/publish`,
          {
            description: publishForm.description,
            category: publishForm.category || "custom",
            priority: publishForm.priority || 100,
            note: publishForm.note,
            script: publishForm.script,
          },
        );
        setPublishModal(null);
        await nerveResult({
          type: "success",
          title: "已发布",
          message: `${name} · ${verLabel(res.version ?? "")}`,
        });
      }
      await load();
    } catch (e) {
      await nerveResult({
        type: "error",
        title: isNew ? "创建失败" : "发布失败",
        message: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setSavingPublish(false);
    }
  };

  const toggleEnabled = async (p: L7PluginSummary) => {
    const enabled = !p.enabled;
    const ok = await nerveConfirm({
      title: enabled ? "启用插件" : "禁用插件",
      message: `确认${enabled ? "启用" : "禁用"}插件「${p.name}」？`,
      okText: "确认",
    });
    if (!ok) return;
    try {
      await apiPut(`/api/plugins/l7/${encodeURIComponent(p.name)}`, { enabled });
      toast(`${enabled ? "已启用" : "已禁用"} ${p.name}`);
      await load();
    } catch (e) {
      await nerveResult({ type: "error", title: "操作失败", message: String(e) });
    }
  };

  const deletePlugin = async (name: string) => {
    const ok = await nerveConfirm({
      title: "删除插件",
      message: `确认删除插件「${name}」？`,
      danger: true,
      okText: "删除",
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/plugins/l7/${encodeURIComponent(name)}`);
      await nerveResult({ type: "success", title: "已删除", message: name });
      await load();
    } catch (e) {
      await nerveResult({ type: "error", title: "删除失败", message: String(e) });
    }
  };

  const openHistory = async (name: string) => {
    try {
      const data = await apiGet<{ current: string; versions: VersionRow[] }>(
        `/api/plugins/l7/${encodeURIComponent(name)}/versions`,
      );
      setHistoryModal({ name, current: data.current, versions: data.versions ?? [] });
    } catch (e) {
      toast(String(e), false);
    }
  };

  const rollback = async (name: string, version: string) => {
    const ok = await nerveConfirm({
      title: "回滚确认",
      message: `将以 ${verLabel(version)} 的内容发布为新版本，是否继续？`,
      okText: "确认回滚",
    });
    if (!ok) return;
    try {
      const res = await apiPost<{ version?: string }>(
        `/api/plugins/l7/${encodeURIComponent(name)}/rollback`,
        { version },
      );
      setHistoryModal(null);
      await nerveResult({
        type: "success",
        title: "已回滚发布",
        message: `${name} · ${verLabel(res.version ?? "")}`,
      });
      await load();
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "回滚失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const currentPublishVersion =
    publishModal && !publishModal.isNew && publishModal.name
      ? plugins.find((p) => p.name === publishModal.name)?.version
      : undefined;

  return (
    <>
      <NervePageHeader
        title={meta.title}
        subtitle={meta.subtitle}
        actions={
          <NerveSyncActions>
            <NerveButton variant="primary" onClick={() => void openPublish(null)}>
              新建
            </NerveButton>
            <NerveButton onClick={() => void load()}>刷新</NerveButton>
          </NerveSyncActions>
        }
      />

      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading && plugins.length === 0 ? <NerveEmpty description="暂无 LUA 插件" /> : null}

      {!loading && plugins.length > 0 ? (
        <div className="l7p-list">
          {plugins.map((p) => (
            <div key={p.name} className="l7p-row">
              <div className="l7p-row-main">
                <button
                  type="button"
                  className="l7p-name ant-btn-link"
                  onClick={() => void openView(p.name)}
                >
                  {p.name}
                </button>
                <span className="l7p-ver-badge">
                  <NerveTag tone="blue">当前版本</NerveTag> <code>{verLabel(p.version)}</code>
                </span>
                <span className="l7p-meta">
                  {p.category ? <NerveTag tone="blue">{p.category}</NerveTag> : null}
                  <NerveTag tone={p.enabled ? "green" : "default"}>
                    {p.enabled ? "启用" : "禁用"}
                  </NerveTag>
                </span>
              </div>
              <div className="l7p-row-time muted small">{fmtTime(updatedAtOf(p))}</div>
              <div className="l7p-row-ops">
                <NerveButton size="sm" variant="link" onClick={() => void openPublish(p.name)}>
                  更新
                </NerveButton>
                <div
                  className={`l7p-more${moreOpen === p.name ? " open" : ""}`}
                  onClick={(e) => e.stopPropagation()}
                >
                  <NerveButton
                    size="sm"
                    variant="link"
                    onClick={() => setMoreOpen((cur) => (cur === p.name ? null : p.name))}
                  >
                    更多 ▾
                  </NerveButton>
                  {moreOpen === p.name ? (
                    <div className="l7p-more-menu">
                      <button type="button" onClick={() => void openHistory(p.name)}>
                        历史版本
                      </button>
                      <button type="button" onClick={() => void toggleEnabled(p)}>
                        {p.enabled ? "禁用" : "启用"}
                      </button>
                      <button
                        type="button"
                        className="danger"
                        onClick={() => void deletePlugin(p.name)}
                      >
                        删除
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
              {p.description ? <p className="muted small">{p.description}</p> : null}
            </div>
          ))}
        </div>
      ) : null}

      <NerveModal
        open={!!viewModal}
        title={viewModal?.title ?? "查看"}
        onClose={() => setViewModal(null)}
        wide
        footer={
          <div>
            <NerveButton onClick={() => setViewModal(null)}>关闭</NerveButton>
          </div>
        }
      >
        {viewModal ? (
          <NerveForm>
            <NerveFormItem label="版本">
              <code>
                {verLabel(viewModal.version ?? viewModal.detail.manifest.version)}
              </code>
            </NerveFormItem>
            <NerveFormItem label="说明">
              <NerveInput wide disabled value={viewModal.detail.manifest.description ?? ""} />
            </NerveFormItem>
            <NerveFormItem label="分类">
              <NerveInput disabled value={viewModal.detail.manifest.category ?? ""} />
            </NerveFormItem>
            {!viewModal.version ? (
              <NerveFormItem label="优先级">
                <NerveInput disabled value={String(viewModal.detail.manifest.priority ?? "")} />
              </NerveFormItem>
            ) : null}
            <NerveFormItem label="Lua 脚本">
              <NerveTextarea
                wide
                rows={16}
                className="mono"
                readOnly
                value={viewModal.detail.script ?? ""}
              />
            </NerveFormItem>
          </NerveForm>
        ) : null}
      </NerveModal>

      <NerveModal
        open={!!publishModal}
        title={
          publishModal?.isNew
            ? "新建 Lua 插件"
            : `更新 · ${publishModal?.name ?? ""}`
        }
        onClose={() => setPublishModal(null)}
        wide
        footer={
          <div>
            <NerveButton onClick={() => setPublishModal(null)}>取消</NerveButton>
            <NerveButton
              variant="primary"
              disabled={savingPublish}
              onClick={() => void submitPublish()}
            >
              {savingPublish ? "提交中…" : publishModal?.isNew ? "创建" : "发布"}
            </NerveButton>
          </div>
        }
      >
        <NerveForm>
          <NerveFormItem label="名称">
            <NerveInput
              wide
              value={publishForm.name}
              placeholder="my-plugin"
              disabled={!publishModal?.isNew}
              onChange={(e) => setPublishForm((f) => ({ ...f, name: e.target.value }))}
            />
          </NerveFormItem>
          {!publishModal?.isNew ? (
            <NerveFormItem label="当前版本">
              <code>{verLabel(currentPublishVersion ?? "")}</code>
            </NerveFormItem>
          ) : null}
          <NerveFormItem label="说明">
            <NerveInput
              wide
              value={publishForm.description}
              onChange={(e) => setPublishForm((f) => ({ ...f, description: e.target.value }))}
            />
          </NerveFormItem>
          <NerveFormItem label="分类">
            <NerveInput
              value={publishForm.category}
              placeholder="xss / sqli / custom"
              onChange={(e) => setPublishForm((f) => ({ ...f, category: e.target.value }))}
            />
          </NerveFormItem>
          <NerveFormItem label="优先级">
            <NerveInput
              type="number"
              value={publishForm.priority}
              onChange={(e) =>
                setPublishForm((f) => ({ ...f, priority: Number(e.target.value) || 100 }))
              }
            />
          </NerveFormItem>
          {!publishModal?.isNew ? (
            <NerveFormItem label="发布说明">
              <NerveInput
                wide
                value={publishForm.note}
                placeholder="可选"
                onChange={(e) => setPublishForm((f) => ({ ...f, note: e.target.value }))}
              />
            </NerveFormItem>
          ) : null}
          <NerveFormItem label="Lua 脚本">
            <NerveTextarea
              wide
              rows={16}
              className="mono"
              value={publishForm.script}
              onChange={(e) => setPublishForm((f) => ({ ...f, script: e.target.value }))}
            />
          </NerveFormItem>
        </NerveForm>
      </NerveModal>

      <NerveModal
        open={!!historyModal}
        title={historyModal ? `历史版本 · ${historyModal.name}` : "历史版本"}
        onClose={() => setHistoryModal(null)}
        wide
        footer={
          <div>
            <NerveButton onClick={() => setHistoryModal(null)}>关闭</NerveButton>
          </div>
        }
      >
        {historyModal && historyModal.versions.length === 0 ? (
          <NerveEmpty description="暂无历史版本" />
        ) : null}
        {historyModal && historyModal.versions.length > 0 ? (
          <NerveTable<VersionRow>
            rowKey="version"
            dataSource={historyModal.versions}
            columns={[
              {
                key: "version",
                title: "版本",
                render: (v) => {
                  const isCur = v.current === true || v.version === historyModal.current;
                  return (
                    <>
                      <code>{verLabel(v.version)}</code>
                      {isCur ? (
                        <>
                          {" "}
                          <NerveTag tone="blue">当前</NerveTag>
                        </>
                      ) : null}
                    </>
                  );
                },
              },
              {
                key: "time",
                title: "时间",
                render: (v) => (
                  <span className="muted small">{fmtTime(v.publishedAt ?? v.published_at)}</span>
                ),
              },
              {
                key: "note",
                title: "说明",
                render: (v) => v.note || "—",
              },
              {
                key: "actions",
                title: "操作",
                className: NERVE_COL_ACTIONS,
                render: (v) => {
                  const isCur = v.current === true || v.version === historyModal.current;
                  return (
                    <NerveTableActions
                      actions={[
                        {
                          key: "view",
                          label: "查看",
                          onClick: () => void openView(historyModal.name, v.version),
                        },
                        ...(isCur
                          ? []
                          : [
                              {
                                key: "rollback",
                                label: "回滚到该版本",
                                onClick: () => void rollback(historyModal.name, v.version),
                              },
                            ]),
                      ]}
                    />
                  );
                },
              },
            ]}
          />
        ) : null}
      </NerveModal>
    </>
  );
}
