/** Console page: named protection strategies and module snapshots. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NervePageHeader,
  NerveSyncActions,
  NerveTag,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiDelete, apiGet, apiPost, apiPut } from "../api";
import { useToast } from "../hooks/useToast";
import {
  CreateStrategyModal,
  StrategyEditor,
} from "../features/protection-strategy";
import {
  expertLevelLabel,
  isAppPrivateStrategy,
  isStrategyEnabled,
  semanticModeLabel,
  strategyScopeLabel,
  toneForSemanticMode,
  type StrategyTemplate,
} from "../lib/strategy";

function tierClass(name: string): string {
  if (name.includes("宽松")) return "pl-tier-relaxed";
  if (name.includes("严格")) return "pl-tier-strict";
  return "pl-tier-standard";
}

export function ProtectionLevelsPage() {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [templates, setTemplates] = useState<StrategyTemplate[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const r = await apiGet<{ templates: StrategyTemplate[] }>("/api/protection-strategies");
      setTemplates(r.templates ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const editing = editingId ? templates.find((t) => t.id === editingId) ?? null : null;

  const upsertTemplate = (t: StrategyTemplate) => {
    setTemplates((prev) => {
      const i = prev.findIndex((x) => x.id === t.id);
      if (i < 0) return [...prev, t];
      const next = [...prev];
      next[i] = t;
      return next;
    });
  };

  const handleCreated = (t: StrategyTemplate) => {
    upsertTemplate(t);
    setEditingId(t.id);
    toast(`「${t.name}」已创建`);
  };

  const handleApplyGlobal = async (tpl: StrategyTemplate) => {
    if (tpl.site_id) return;
    const ok = await nerveConfirm({
      title: "设为全局默认",
      message: `将「${tpl.name}」套用为全局默认防护配置？各应用未单独绑定时将继承。`,
      okText: "设为全局",
    });
    if (!ok) return;
    try {
      await apiPost(`/api/protection-strategies/${encodeURIComponent(tpl.id)}/apply`, {
        target: "global",
      });
      toast("已设为全局默认");
    } catch (e) {
      await nerveResult({ type: "error", title: "操作失败", message: String(e) });
    }
  };

  const handleDelete = async (tpl: StrategyTemplate) => {
    const ok = await nerveConfirm({
      title: "删除策略",
      message: `确定删除「${tpl.name}」？`,
      okText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/protection-strategies/${encodeURIComponent(tpl.id)}`);
      setTemplates((prev) => prev.filter((t) => t.id !== tpl.id));
      if (editingId === tpl.id) setEditingId(null);
      toast("已删除");
    } catch (e) {
      await nerveResult({ type: "error", title: "删除失败", message: String(e) });
    }
  };

  const handleToggleEnabled = async (tpl: StrategyTemplate) => {
    if (!isAppPrivateStrategy(tpl)) return;
    const next = !isStrategyEnabled(tpl);
    const ok = await nerveConfirm({
      title: next ? "启用策略" : "禁用策略",
      message: next
        ? `启用「${tpl.name}」？`
        : `禁用「${tpl.name}」？禁用后不可再选绑；若当前绑定该策略将回退到「标准」。`,
      okText: next ? "启用" : "禁用",
      danger: !next,
    });
    if (!ok) return;
    try {
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(tpl.id)}`,
        { enabled: next },
      );
      if (resp.template) upsertTemplate(resp.template);
      toast(next ? "已启用" : "已禁用");
    } catch (e) {
      await nerveResult({ type: "error", title: "操作失败", message: String(e) });
    }
  };

  if (editing) {
    return (
      <div className="pl-page">
        <StrategyEditor
          template={editing}
          onBack={() => setEditingId(null)}
          onSaved={upsertTemplate}
          toast={toast}
        />
      </div>
    );
  }

  return (
    <div className="pl-page">
      <NervePageHeader
        title="防护策略"
        actions={
          <NerveSyncActions>
            <NerveButton variant="primary" onClick={() => setCreateOpen(true)}>
              新建策略
            </NerveButton>
            <NerveButton onClick={() => void load()}>刷新</NerveButton>
          </NerveSyncActions>
        }
      />

      {error ? <NerveAlert tone="error">加载策略库失败：{error}</NerveAlert> : null}
      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading && templates.length === 0 ? (
        <div className="pl-empty">
          <span className="muted">暂无策略</span>
          <NerveButton variant="primary" size="sm" onClick={() => setCreateOpen(true)}>
            新建策略
          </NerveButton>
        </div>
      ) : null}

      {!loading && templates.length > 0 ? (
        <div className="pl-strategy-grid">
          {templates.map((t) => {
            const enabled = isStrategyEnabled(t);
            const appPrivate = isAppPrivateStrategy(t);
            return (
              <div
                key={t.id}
                className={`pl-strategy-card ${tierClass(t.name)}${enabled ? "" : " is-disabled"}`}
              >
                <div className="pl-strategy-card-accent" />
                <div className="pl-strategy-card-body">
                  <div className="pl-strategy-card-top">
                    <h3 className="pl-strategy-name">{t.name}</h3>
                    <NerveTag tone={t.builtin ? "blue" : t.site_id ? "green" : "default"}>
                      {t.builtin ? "内置" : strategyScopeLabel(t)}
                      {!enabled ? " · 已禁用" : ""}
                    </NerveTag>
                  </div>
                  <dl className="pl-strategy-kv">
                    <div>
                      <dt>语义</dt>
                      <dd>
                        <NerveTag tone={toneForSemanticMode(t.semantic_mode)}>
                          {semanticModeLabel(t.semantic_mode)}
                        </NerveTag>
                      </dd>
                    </div>
                    <div>
                      <dt>专家</dt>
                      <dd>
                        <NerveTag>{expertLevelLabel(t.expert_level)}</NerveTag>
                      </dd>
                    </div>
                  </dl>
                  {t.description ? <p className="muted small">{t.description}</p> : null}
                  <div className="pl-strategy-card-foot">
                    <NerveButton
                      size="sm"
                      variant="primary"
                      onClick={() => setEditingId(t.id)}
                    >
                      编辑
                    </NerveButton>
                    {!t.builtin && !t.site_id ? (
                      <NerveButton size="sm" onClick={() => void handleApplyGlobal(t)}>
                        设为全局
                      </NerveButton>
                    ) : null}
                    {appPrivate ? (
                      <NerveButton size="sm" onClick={() => void handleToggleEnabled(t)}>
                        {enabled ? "禁用" : "启用"}
                      </NerveButton>
                    ) : null}
                    {!t.builtin ? (
                      <NerveButton size="sm" variant="danger" onClick={() => void handleDelete(t)}>
                        删除
                      </NerveButton>
                    ) : null}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      ) : null}

      <CreateStrategyModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={handleCreated}
      />
    </div>
  );
}
