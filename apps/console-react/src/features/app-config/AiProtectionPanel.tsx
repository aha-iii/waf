import { NerveSwitch, NerveTag } from "@nerve/ui";
import type { SemanticConfig } from "../../lib/semantic";
import { normalizePipelineEnabled, normalizePipelineOrder } from "../../lib/pipeline";

type Props = {
  cfg: SemanticConfig;
  readonly?: boolean;
  onChange: (next: SemanticConfig) => void;
};

export function applyAiProtectionEnabled(cfg: SemanticConfig, enabled: boolean): SemanticConfig {
  const order = normalizePipelineOrder(cfg.engines.pipeline_order ?? []);
  const pipelineEnabled = normalizePipelineEnabled(cfg.engines.pipeline_enabled);
  return {
    ...cfg,
    ai_protection: { enabled },
    engines: {
      ...cfg.engines,
      pipeline_order: order.includes("ai") ? order : [...order, "ai"],
      pipeline_enabled: { ...pipelineEnabled, ai: enabled },
    },
  };
}

/** AI 防护：面向 AI 应用本身的威胁（Prompt 注入等）。 */
export function AiProtectionPanel({ cfg, readonly, onChange }: Props) {
  const enabled = cfg.ai_protection?.enabled ?? false;

  return (
    <div className="module-grid app-config-mod-grid">
      <div className="protection-module-card pl-ai-card">
        <div className="protection-module-head">
          <strong>AI防护</strong>
          <NerveSwitch
            checked={enabled}
            disabled={readonly}
            onChange={(e) => onChange(applyAiProtectionEnabled(cfg, e.target.checked))}
          />
        </div>
        <div className="protection-module-actions" style={{ marginTop: 12 }}>
          <NerveTag tone={enabled ? "blue" : "default"}>
            {enabled ? "只观测 · 不拦截" : "未启用"}
          </NerveTag>
        </div>
      </div>
    </div>
  );
}
