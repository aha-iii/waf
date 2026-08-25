/** protection-strategy feature: Custom Sets Modal. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveModal,
  NerveTable,
  NerveTag,
  nerveResult,
} from "@nerve/ui";
import { apiGet, apiPut } from "../../api";
import type { CustomRuleSetInfo } from "../../module-meta";
import {
  buildCoreWithCustomSets,
  strategyCustomSetIds,
  type StrategyTemplate,
} from "../../lib/strategy";

type SetRow = CustomRuleSetInfo & Record<string, unknown>;

type Props = {
  open: boolean;
  template: StrategyTemplate;
  onClose: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

export function CustomSetsModal({ open, template, onClose, onSaved, toast }: Props) {
  const [allSets, setAllSets] = useState<CustomRuleSetInfo[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const loadSets = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const url = template.site_id
        ? `/api/custom-rules?site_id=${encodeURIComponent(template.site_id)}`
        : "/api/global/custom-rules";
      const res = await apiGet<{ sets: CustomRuleSetInfo[] }>(url);
      setAllSets(res.sets ?? []);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [template.site_id]);

  useEffect(() => {
    if (!open) return;
    setSelectedIds(strategyCustomSetIds(template));
    setPickerOpen(false);
    void loadSets();
  }, [open, template, loadSets]);

  const selectedRows = useMemo((): SetRow[] => {
    return selectedIds.map((id) => {
      const hit = allSets.find((s) => s.id === id);
      return (hit ?? { id, name: id, description: "", enabled: true }) as SetRow;
    });
  }, [selectedIds, allSets]);

  const availableRows = useMemo((): SetRow[] => {
    return allSets.filter((s) => !selectedIds.includes(s.id)) as SetRow[];
  }, [allSets, selectedIds]);

  const detach = (id: string) => {
    setSelectedIds((prev) => prev.filter((x) => x !== id));
  };

  const attach = (id: string) => {
    setSelectedIds((prev) => (prev.includes(id) ? prev : [...prev, id]));
  };

  const save = async () => {
    setSaving(true);
    try {
      const core = buildCoreWithCustomSets(template.core, selectedIds);
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(template.id)}`,
        {
          custom: { set_ids: selectedIds },
          core,
        },
      );
      if (resp.template) onSaved(resp.template);
      toast("已保存");
      onClose();
    } catch (e) {
      await nerveResult({ type: "error", title: "保存失败", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const columns = [
    {
      key: "name",
      title: "规则集",
      render: (row: SetRow) => (
        <>
          <strong>{row.name || row.id}</strong>
          <div className="muted small">
            <code>{row.id}</code>
          </div>
        </>
      ),
    },
    {
      key: "description",
      title: "说明",
      render: (row: SetRow) => <span className="muted small">{row.description || "—"}</span>,
    },
    {
      key: "status",
      title: "状态",
      render: (row: SetRow) =>
        row.enabled === false ? (
          <NerveTag tone="orange">已停用</NerveTag>
        ) : (
          <NerveTag tone="green">可用</NerveTag>
        ),
    },
  ];

  return (
    <>
      <NerveModal
        open={open}
        title={`自定义防护 · ${template.name}`}
        onClose={onClose}
        wide
        footer={
          <>
            <NerveButton onClick={onClose}>取消</NerveButton>
            <NerveButton variant="primary" disabled={saving || loading} onClick={() => void save()}>
              {saving ? "保存中…" : "保存"}
            </NerveButton>
          </>
        }
      >
        {error ? <NerveAlert tone="error">加载规则集失败：{error}</NerveAlert> : null}
        {loading ? <div className="page-loading">加载规则集…</div> : null}
        {!loading && !error ? (
          <div className="pl-modal-section">
            <div className="pl-modal-toolbar">
              <NerveButton size="sm" variant="primary" onClick={() => setPickerOpen(true)}>
                添加
              </NerveButton>
            </div>
            <NerveTable<SetRow>
              rowKey="id"
              emptyText="尚未绑定自定义规则集，请点击「添加」从已有规则集中选择。"
              dataSource={selectedRows}
              columns={[
                ...columns,
                {
                  key: "ops",
                  title: "操作",
                  render: (row) => (
                    <NerveButton size="sm" onClick={() => detach(row.id)}>
                      移除
                    </NerveButton>
                  ),
                },
              ]}
            />
            <p className="muted small" style={{ marginTop: 12 }}>
              本页仅添加/移除已有规则集到策略快照；新建与编辑请前往「签名规则」。
            </p>
          </div>
        ) : null}
      </NerveModal>

      <NerveModal
        open={pickerOpen}
        title="添加自定义规则集"
        onClose={() => setPickerOpen(false)}
        wide
        footer={<NerveButton onClick={() => setPickerOpen(false)}>关闭</NerveButton>}
      >
        <div className="pl-modal-section">
          <p className="muted small" style={{ margin: "0 0 12px" }}>
            来自自定义防护 / 签名规则中的规则集，选择后挂到本策略快照。
          </p>
          <NerveTable<SetRow>
            rowKey="id"
            emptyText={
              allSets.length
                ? "可添加的规则集已全部挂到本策略。"
                : "暂无自定义规则集，请先到「签名规则」创建。"
            }
            dataSource={availableRows}
            columns={[
              ...columns,
              {
                key: "ops",
                title: "操作",
                render: (row) => (
                  <NerveButton size="sm" variant="primary" onClick={() => attach(row.id)}>
                    添加
                  </NerveButton>
                ),
              },
            ]}
          />
        </div>
      </NerveModal>
    </>
  );
}
