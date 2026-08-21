/** protection-strategy feature: Pre Access Stage Modal. */
import { useEffect, useMemo, useState } from "react";
import { NerveButton, NerveModal, nerveResult } from "@nerve/ui";
import { apiPut } from "../../api";
import {
  orderedPreAccessModuleEntries,
  orderedResponseModuleEntries,
} from "../../module-meta";
import type { StrategyModuleCfg, StrategyTemplate } from "../../lib/strategy";
import { BasicModuleDetailModal } from "../modules/BasicModuleDetailModal";
import { copyBasicModuleFields } from "../modules/basic-module-params";
import { CoreModuleSection } from "../modules/CoreModuleCard";
import { basicModuleDetailSupported } from "../modules/basic-module-detail";

type Props = {
  open: boolean;
  template: StrategyTemplate;
  onClose: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

export function PreAccessStageModal({ open, template, onClose, onSaved, toast }: Props) {
  const core = template.core ?? { version: 1, mode: "block", modules: {} };
  const [modules, setModules] = useState<Record<string, StrategyModuleCfg>>({
    ...(core.modules ?? {}),
  });
  const [saving, setSaving] = useState(false);
  const [detailKey, setDetailKey] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const c = template.core ?? { version: 1, mode: "block", modules: {} };
    setModules({ ...(c.modules ?? {}) });
  }, [open, template]);

  const preAccessRows = useMemo(() => orderedPreAccessModuleEntries(modules), [modules]);
  const responseRows = useMemo(() => orderedResponseModuleEntries(modules), [modules]);
  const stageRows = useMemo(
    () => [...preAccessRows, ...responseRows],
    [preAccessRows, responseRows],
  );

  const updateMod = (key: string, patch: Partial<StrategyModuleCfg>) => {
    setModules((prev) => {
      const cur = prev[key] ?? { enabled: false, action: "block", sensitivity: "medium" };
      return { ...prev, [key]: { ...cur, ...patch } };
    });
  };

  const save = async () => {
    setSaving(true);
    try {
      const nextModules: Record<string, StrategyModuleCfg> = { ...(core.modules ?? {}), ...modules };
      for (const [key, cfg] of stageRows) {
        const prev = modules[key];
        nextModules[key] = {
          enabled: cfg.enabled,
          action: cfg.action,
          sensitivity: cfg.sensitivity,
          disabled_rules: prev?.disabled_rules,
          ...copyBasicModuleFields(prev ?? cfg),
        };
      }
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(template.id)}`,
        {
          core: {
            version: core.version ?? 1,
            mode: core.mode ?? "block",
            modules: nextModules,
          },
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

  return (
    <NerveModal
      open={open}
      title={`基础防护 · ${template.name}`}
      onClose={onClose}
      wide
      footer={
        <>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" disabled={saving} onClick={() => void save()}>
            {saving ? "保存中…" : "保存"}
          </NerveButton>
        </>
      }
    >
      <CoreModuleSection
        entries={preAccessRows}
        configScope="strategy"
        onChange={updateMod}
        onOpenDetail={(key) => {
          if (basicModuleDetailSupported(key)) setDetailKey(key);
        }}
      />
      <CoreModuleSection
        entries={responseRows}
        configScope="strategy"
        onChange={updateMod}
        onOpenDetail={(key) => {
          if (basicModuleDetailSupported(key)) setDetailKey(key);
        }}
      />

      {detailKey ? (
        <BasicModuleDetailModal
          open
          moduleKey={detailKey}
          cfg={
            modules[detailKey] ?? { enabled: false, action: "block", sensitivity: "medium" }
          }
          configScope="strategy"
          onClose={() => setDetailKey(null)}
          onChange={(patch) => updateMod(detailKey, patch)}
        />
      ) : null}
    </NerveModal>
  );
}
