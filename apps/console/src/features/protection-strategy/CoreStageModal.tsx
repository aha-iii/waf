/** protection-strategy feature: Core Stage Modal. */
import { useEffect, useMemo, useState } from "react";
import {
  NerveButton,
  NerveModal,
  NerveSelect,
  nerveResult,
} from "@nerve/ui";
import { apiPut } from "../../api";
import { orderedWebAttackModuleEntries } from "../../module-meta";
import type { StrategyModuleCfg, StrategyTemplate } from "../../lib/strategy";
import { CoreModuleSection } from "../modules/CoreModuleCard";
import { moduleScopeHint } from "../modules/module-config-scope";

type Props = {
  open: boolean;
  template: StrategyTemplate;
  onClose: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

export function CoreStageModal({ open, template, onClose, onSaved, toast }: Props) {
  const core = template.core ?? { version: 1, mode: "block", modules: {} };
  const [mode, setMode] = useState(core.mode ?? "block");
  const [modules, setModules] = useState<Record<string, StrategyModuleCfg>>({
    ...(core.modules ?? {}),
  });
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    const c = template.core ?? { version: 1, mode: "block", modules: {} };
    setMode(c.mode ?? "block");
    setModules({ ...(c.modules ?? {}) });
  }, [open, template]);

  const attackRows = useMemo(() => orderedWebAttackModuleEntries(modules), [modules]);

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
      for (const [key, cfg] of attackRows) {
        const prev = modules[key];
        nextModules[key] = {
          enabled: cfg.enabled,
          action: cfg.action,
          sensitivity: cfg.sensitivity,
          disabled_rules: prev?.disabled_rules,
        };
      }
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(template.id)}`,
        {
          core: {
            version: core.version ?? 1,
            mode,
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
      title={`核心防护 · ${template.name}`}
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
      <p className="muted small" style={{ marginBottom: 16 }}>
        {moduleScopeHint("strategy")}
      </p>
      <div className="pl-core-mode-row">
        <span className="pl-modal-label">默认动作</span>
        <NerveSelect
          value={mode}
          options={[
            { value: "block", label: "拦截" },
            { value: "detect", label: "仅检测" },
          ]}
          style={{ width: 140 }}
          onChange={(e) => setMode(e.target.value)}
        />
      </div>
      <CoreModuleSection
        label="攻击检测"
        hint="按攻击类型匹配请求特征，可配置灵敏度与单条规则。"
        entries={attackRows}
        configScope="strategy"
        onChange={updateMod}
      />
    </NerveModal>
  );
}
