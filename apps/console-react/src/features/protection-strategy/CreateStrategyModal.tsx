import { useEffect, useState } from "react";
import {
  NerveButton,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  nerveResult,
} from "@nerve/ui";
import { apiPost } from "../../api";
import { useToast } from "../../hooks/useToast";
import type { StrategyTemplate } from "../../lib/strategy";

type Props = {
  open: boolean;
  onClose: () => void;
  onCreated: (t: StrategyTemplate) => void;
};

export function CreateStrategyModal({ open, onClose, onCreated }: Props) {
  const toast = useToast();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setName("");
      setDescription("");
    }
  }, [open]);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      toast("请填写名称", false);
      return;
    }
    setSaving(true);
    try {
      const resp = await apiPost<{ ok: boolean; template: StrategyTemplate }>(
        "/api/protection-strategies",
        { name: trimmed, description: description.trim() || "" },
      );
      if (!resp.template) throw new Error("未返回策略");
      onCreated(resp.template);
      onClose();
    } catch (e) {
      await nerveResult({ type: "error", title: "创建失败", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <NerveModal
      open={open}
      title="新建策略"
      onClose={onClose}
      footer={
        <>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" disabled={saving} onClick={() => void submit()}>
            {saving ? "创建中…" : "创建"}
          </NerveButton>
        </>
      }
    >
      <NerveForm>
        <NerveFormItem label="名称" required>
          <NerveInput
            wide
            value={name}
            placeholder="请输入名称"
            onChange={(e) => setName(e.target.value)}
          />
        </NerveFormItem>
        <NerveFormItem label="说明">
          <NerveInput
            wide
            value={description}
            placeholder="可选"
            onChange={(e) => setDescription(e.target.value)}
          />
        </NerveFormItem>
      </NerveForm>
      <p className="muted small" style={{ marginTop: 8 }}>
        创建后将从当前已保存的全局基础防护配置拷贝快照；已有策略不会随全局变更而更新。
      </p>
    </NerveModal>
  );
}
