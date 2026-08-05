import { useCallback, useEffect, useState } from "react";
import { NerveButton, NerveModal, nerveConfirm, nerveResult } from "@nerve/ui";
import { apiGet, apiPut } from "../../api";
import type { StrategyTemplate } from "../../lib/strategy";
import {
  mergeSemanticConfig,
  semanticFromStrategyTemplate,
  semanticToStrategyJson,
  STRATEGY_SEMANTIC_TABS,
  type SemanticConfig,
} from "../../lib/semantic";
import { SemanticConsole } from "../semantic/SemanticConsole";
import { GLOBAL_DEFAULT_SITE_ID } from "../../sites";

type Props = {
  open: boolean;
  template: StrategyTemplate;
  onClose: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

export function SemanticStageModal({ open, template, onClose, onSaved, toast }: Props) {
  const [cfg, setCfg] = useState<SemanticConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await apiGet<{ config: Partial<SemanticConfig> }>(
        `/api/semantic/config?site_id=${encodeURIComponent(GLOBAL_DEFAULT_SITE_ID)}`,
      );
      const globalCfg = mergeSemanticConfig(res.config);
      setCfg(semanticFromStrategyTemplate(template, globalCfg));
    } catch (e) {
      await nerveResult({ type: "error", title: "加载失败", message: String(e) });
      setCfg(mergeSemanticConfig({ mode: template.semantic_mode as SemanticConfig["mode"] }));
    } finally {
      setLoading(false);
    }
  }, [template]);

  useEffect(() => {
    if (!open) return;
    void load();
  }, [open, load]);

  const publish = async () => {
    if (!cfg) return;
    const ok = await nerveConfirm({
      title: "发布",
      message: `确认发布「${template.name}」的语义设置？应用到站点后生效。`,
      okText: "发布",
    });
    if (!ok) return;
    setSaving(true);
    try {
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(template.id)}`,
        {
          semantic_mode: cfg.mode,
          semantic: semanticToStrategyJson(cfg),
        },
      );
      if (resp.template) onSaved(resp.template);
      toast("已发布");
      onClose();
    } catch (e) {
      await nerveResult({ type: "error", title: "发布失败", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <NerveModal
      open={open}
      title={`语义设置 · ${template.name}`}
      onClose={onClose}
      wide
      footer={
        <>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" disabled={saving || loading || !cfg} onClick={() => void publish()}>
            {saving ? "发布中…" : "发布"}
          </NerveButton>
        </>
      }
    >
      {loading || !cfg ? (
        <p className="muted small" style={{ padding: "24px 0", textAlign: "center" }}>
          加载语义配置…
        </p>
      ) : (
        <SemanticConsole
          compact
          cfg={cfg}
          onChange={setCfg}
          visibleTabs={STRATEGY_SEMANTIC_TABS}
        />
      )}
    </NerveModal>
  );
}
