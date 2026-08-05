import { useEffect, useState } from "react";
import {
  NerveButton,
  NerveEmpty,
  NerveModal,
  NerveSwitch,
  NerveTable,
  NerveTag,
} from "@nerve/ui";
import { apiGet, apiPut } from "../../api";
import { siteQuery } from "../../sites";
import { useToast } from "../../hooks/useToast";

type CustomRuleRow = {
  id: string;
  rule_set_id: string;
  name: string;
  rule_type?: string;
  match_field: string;
  match_op: string;
  match_pattern: string;
  action: string;
  priority: number;
  enabled: boolean;
};

type Payload = {
  sets: { id: string; name: string; description?: string; enabled: boolean }[];
  rules: CustomRuleRow[];
};

type Props = {
  open: boolean;
  setId: string;
  setName: string;
  siteId?: string;
  editable: boolean;
  onClose: () => void;
  onSaved?: () => void;
};

export function CustomSetRulesModal({
  open,
  setId,
  setName,
  siteId,
  editable,
  onClose,
  onSaved,
}: Props) {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [payload, setPayload] = useState<Payload | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLoading(true);
    const qs = siteId ? siteQuery(siteId) : "";
    void apiGet<Payload>(`/api/custom-rules${qs}`)
      .then((data) => setPayload(data))
      .catch((e) => {
        toast(String(e), false);
        setPayload({ sets: [], rules: [] });
      })
      .finally(() => setLoading(false));
  }, [open, siteId, toast]);

  const rows = (payload?.rules ?? []).filter((r) => r.rule_set_id === setId);

  const toggle = (id: string, enabled: boolean) => {
    if (!payload || !editable) return;
    setPayload({
      ...payload,
      rules: payload.rules.map((r) => (r.id === id ? { ...r, enabled } : r)),
    });
  };

  const save = async () => {
    if (!editable || !siteId || !payload) return;
    setSaving(true);
    try {
      await apiPut(`/api/custom-rules${siteQuery(siteId)}`, payload);
      toast("规则集已保存");
      onSaved?.();
      onClose();
    } catch (e) {
      toast(String(e), false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <NerveModal
      open={open}
      title={`${setName}${editable ? " · 本应用规则集" : " · 规则预览"}`}
      wide
      onClose={onClose}
      footer={
        editable ? (
          <>
            <NerveButton onClick={onClose}>取消</NerveButton>
            <NerveButton variant="primary" disabled={saving} onClick={() => void save()}>
              {saving ? "保存中…" : "确定"}
            </NerveButton>
          </>
        ) : (
          <NerveButton variant="primary" onClick={onClose}>
            关闭
          </NerveButton>
        )
      }
    >
      {loading ? <div className="page-loading">加载规则…</div> : null}
      {!loading && rows.length === 0 ? <NerveEmpty description="该规则集暂无规则" /> : null}
      {!loading && rows.length > 0 ? (
        <NerveTable
          rowKey="id"
          dataSource={rows}
          columns={[
            { key: "name", title: "名称", render: (r) => r.name },
            {
              key: "type",
              title: "类型",
              render: (r) => (r.rule_type === "plugin" ? "自定义插件" : "签名规则"),
            },
            {
              key: "match",
              title: "匹配",
              render: (r) => (
                <code className="small">
                  {r.match_field} {r.match_op} {r.match_pattern}
                </code>
              ),
            },
            { key: "action", title: "动作", render: (r) => <NerveTag>{r.action}</NerveTag> },
            {
              key: "enabled",
              title: "启用",
              render: (r) =>
                editable ? (
                  <NerveSwitch checked={r.enabled} onChange={(e) => toggle(r.id, e.target.checked)} />
                ) : (
                  <NerveTag tone={r.enabled ? "green" : "default"}>
                    {r.enabled ? "启用" : "停用"}
                  </NerveTag>
                ),
            },
          ]}
        />
      ) : null}
    </NerveModal>
  );
}
