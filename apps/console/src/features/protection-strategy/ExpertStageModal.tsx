/** protection-strategy feature: Expert Stage Modal. */
import { useEffect, useState } from "react";
import { NerveButton, NerveModal, nerveResult } from "@nerve/ui";
import { apiPut } from "../../api";
import { EXPERT_OPTS, type StrategyTemplate } from "../../lib/strategy";
import { OptionCards } from "./OptionCards";

type Props = {
  open: boolean;
  template: StrategyTemplate;
  onClose: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

export function ExpertStageModal({ open, template, onClose, onSaved, toast }: Props) {
  const [level, setLevel] = useState(template.expert_level || "medium");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) setLevel(template.expert_level || "medium");
  }, [open, template.expert_level]);

  const save = async () => {
    setSaving(true);
    try {
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(template.id)}`,
        { expert_level: level },
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

  return (
    <NerveModal
      open={open}
      title={`专家防护 · ${template.name}`}
      onClose={onClose}
      footer={
        <>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" disabled={saving} onClick={() => void save()}>
            {saving ? "保存中…" : "保存"}
          </NerveButton>
        </>
      }
    >
      <div className="pl-modal-section">
        <div className="pl-modal-label">防护强度</div>
        <OptionCards name="pl-expert" options={EXPERT_OPTS} value={level} onChange={setLevel} />
      </div>
    </NerveModal>
  );
}
