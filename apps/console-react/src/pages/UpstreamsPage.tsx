import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveEmpty,
  NervePageHeader,
  NerveTableToolbar,
  NerveTag,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiGet, apiPut } from "../api";
import { UpstreamEditorDrawer } from "../features/upstreams/UpstreamEditorDrawer";
import {
  defaultUpstream,
  nodesSummary,
  normalizeUpstream,
  upstreamOriginKind,
  upstreamOriginKindLabel,
  type UpstreamsResponse,
  type WafUpstream,
} from "../features/upstreams/upstream-types";

export function UpstreamsPage() {
  const [loading, setLoading] = useState(true);
  const [upstreams, setUpstreams] = useState<WafUpstream[]>([]);
  const [editIndex, setEditIndex] = useState<number | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await apiGet<UpstreamsResponse>("/api/upstreams");
      setUpstreams((res.upstreams ?? []).map(normalizeUpstream));
    } catch (e) {
      void nerveResult({
        type: "error",
        title: "加载失败",
        message: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const saveAll = async (next: WafUpstream[]) => {
    const res = await apiPut<UpstreamsResponse>("/api/upstreams", { upstreams: next });
    setUpstreams(res.upstreams ?? next);
    if (res.gateway_sync?.ok === false) {
      void nerveResult({
        type: "error",
        title: "Gateway 同步失败",
        message: res.gateway_sync.error ?? "未知错误",
      });
    }
  };

  const handleSaveOne = async (upstream: WafUpstream) => {
    try {
      const idx = upstreams.findIndex((u) => u.id === upstream.id);
      const next =
        idx >= 0 ? upstreams.map((u, i) => (i === idx ? upstream : u)) : [...upstreams, upstream];
      await saveAll(next);
      setEditIndex(null);
      setCreating(false);
      void nerveResult({ type: "success", title: "已保存", message: upstream.id });
    } catch (e) {
      void nerveResult({
        type: "error",
        title: "保存失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const handleDelete = async (index: number) => {
    const u = upstreams[index];
    const ok = await nerveConfirm({
      title: "删除 Upstream",
      message: `确认删除「${u.name}」？`,
    });
    if (!ok) return;
    try {
      await saveAll(upstreams.filter((_, i) => i !== index));
    } catch (e) {
      void nerveResult({
        type: "error",
        title: "删除失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const editorUpstream =
    creating ? defaultUpstream() : editIndex != null ? upstreams[editIndex] : null;

  return (
    <>
      <NervePageHeader title="Upstreams" />
      <NerveTableToolbar>
        <NerveButton onClick={() => void load()}>刷新</NerveButton>
        <NerveButton
          variant="primary"
          onClick={() => {
            setEditIndex(null);
            setCreating(true);
          }}
        >
          + 创建 Upstream
        </NerveButton>
      </NerveTableToolbar>

      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading && upstreams.length === 0 ? (
        <NerveEmpty description="暂无 Upstream" />
      ) : null}

      {!loading && upstreams.length > 0 ? (
        <div className="ant-table-wrapper">
          <table className="ant-table origin-node-table">
            <thead>
              <tr>
                <th>ID / 名称</th>
                <th>类型</th>
                <th>协议</th>
                <th>节点</th>
                <th>状态</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {upstreams.map((u, i) => (
                <tr key={u.id}>
                  <td>
                    <code>{u.id}</code>
                    <div className="muted small">{u.name}</div>
                  </td>
                  <td>
                    <NerveTag tone={upstreamOriginKind(u) === "gateway" ? "orange" : "blue"}>
                      {upstreamOriginKindLabel(upstreamOriginKind(u))}
                    </NerveTag>
                  </td>
                  <td>
                    <NerveTag tone="blue">{u.scheme.toUpperCase()}</NerveTag>
                  </td>
                  <td className="muted small">
                    {upstreamOriginKind(u) === "gateway" ? "—" : nodesSummary(u.nodes)}
                  </td>
                  <td>
                    <NerveTag tone={u.enabled ? "green" : "default"}>
                      {u.enabled ? "启用" : "禁用"}
                    </NerveTag>
                  </td>
                  <td>
                    <NerveButton variant="link" size="sm" onClick={() => setEditIndex(i)}>
                      编辑
                    </NerveButton>
                    <NerveButton variant="link" size="sm" onClick={() => void handleDelete(i)}>
                      删除
                    </NerveButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}

      {editorUpstream ? (
        <UpstreamEditorDrawer
          open
          upstream={editorUpstream}
          onClose={() => {
            setEditIndex(null);
            setCreating(false);
          }}
          onSave={(u) => void handleSaveOne(u)}
        />
      ) : null}
    </>
  );
}
