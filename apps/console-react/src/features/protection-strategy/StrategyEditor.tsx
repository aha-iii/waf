import { useState } from "react";
import { NerveButton, NerveTag } from "@nerve/ui";
import {
  expertLevelLabel,
  semanticModeLabel,
  strategyCoreAttackSummary,
  strategyCoreEnabled,
  strategyCustomEnabled,
  strategyCustomSetIds,
  strategyExpertEnabled,
  strategyPreAccessEnabled,
  strategyPreAccessSummary,
  strategySemanticEnabled,
  toneForCoreSummary,
  toneForCustomSummary,
  toneForExpertLevel,
  toneForPreAccessSummary,
  toneForSemanticMode,
  type StrategyTemplate,
} from "../../lib/strategy";
import { SemanticStageModal } from "./SemanticStageModal";
import { ExpertStageModal } from "./ExpertStageModal";
import { PreAccessStageModal } from "./PreAccessStageModal";
import { CoreStageModal } from "./CoreStageModal";
import { CustomSetsModal } from "./CustomSetsModal";

type Stage = "semantic" | "pre_access" | "expert" | "core" | "custom" | null;

type Props = {
  template: StrategyTemplate;
  onBack: () => void;
  onSaved: (t: StrategyTemplate) => void;
  toast: (msg: string, ok?: boolean) => void;
};

type StageRow = {
  key: Stage & string;
  title: string;
  icon: string;
  enabled: boolean;
  summary: string;
  summaryTone: "default" | "green" | "red" | "orange" | "blue";
};

export function StrategyEditor({ template, onBack, onSaved, toast }: Props) {
  const [stage, setStage] = useState<Stage>(null);
  const customCount = strategyCustomSetIds(template).length;
  const preAccessSummary = strategyPreAccessSummary(template);
  const coreSummary = strategyCoreAttackSummary(template);

  const settings: StageRow[] = [
    {
      key: "semantic",
      title: "语义设置",
      icon: "语",
      enabled: strategySemanticEnabled(template),
      summary: semanticModeLabel(template.semantic_mode),
      summaryTone: toneForSemanticMode(template.semantic_mode),
    },
  ];

  const modules: StageRow[] = [
    {
      key: "pre_access",
      title: "基础防护",
      icon: "基",
      enabled: strategyPreAccessEnabled(template),
      summary: preAccessSummary,
      summaryTone: toneForPreAccessSummary(preAccessSummary),
    },
    {
      key: "expert",
      title: "专家防护",
      icon: "专",
      enabled: strategyExpertEnabled(template),
      summary: expertLevelLabel(template.expert_level),
      summaryTone: toneForExpertLevel(template.expert_level),
    },
    {
      key: "core",
      title: "核心防护",
      icon: "核",
      enabled: strategyCoreEnabled(template),
      summary: coreSummary,
      summaryTone: toneForCoreSummary(coreSummary),
    },
    {
      key: "custom",
      title: "自定义防护",
      icon: "自",
      enabled: strategyCustomEnabled(template),
      summary: customCount ? `${customCount} 个规则集` : "未挂载",
      summaryTone: toneForCustomSummary(customCount > 0),
    },
  ];

  const renderRows = (rows: StageRow[]) =>
    rows.map((s) => (
      <button
        key={s.key}
        type="button"
        className={`pl-stage-row${s.enabled ? "" : " is-disabled"}`}
        onClick={() => setStage(s.key as Stage)}
      >
        <span className={`pl-stage-icon${s.enabled ? " is-on" : ""}`}>{s.icon}</span>
        <span className="pl-stage-main">
          <strong>{s.title}</strong>
        </span>
        <span className="pl-stage-value">
          <NerveTag tone={s.summaryTone}>{s.summary}</NerveTag>
        </span>
        <span className="pl-stage-chevron">›</span>
      </button>
    ));

  return (
    <div className="pl-edit-page" id="pl-tpl-editor">
      <div className="pl-edit-bar">
        <NerveButton size="sm" onClick={onBack}>
          返回
        </NerveButton>
        <div className="pl-edit-title">
          <h2>{template.name}</h2>
          {template.builtin ? (
            <NerveTag tone="blue">内置</NerveTag>
          ) : (
            <NerveTag>自定义</NerveTag>
          )}
        </div>
      </div>

      <p className="pl-section-label">防护设置</p>
      <div className="pl-stage-list">{renderRows(settings)}</div>

      <p className="pl-section-label" style={{ marginTop: 24 }}>
        防护模块
      </p>
      <div className="pl-stage-list">{renderRows(modules)}</div>

      <SemanticStageModal
        open={stage === "semantic"}
        template={template}
        onClose={() => setStage(null)}
        onSaved={onSaved}
        toast={toast}
      />
      <PreAccessStageModal
        open={stage === "pre_access"}
        template={template}
        onClose={() => setStage(null)}
        onSaved={onSaved}
        toast={toast}
      />
      <ExpertStageModal
        open={stage === "expert"}
        template={template}
        onClose={() => setStage(null)}
        onSaved={onSaved}
        toast={toast}
      />
      <CoreStageModal
        open={stage === "core"}
        template={template}
        onClose={() => setStage(null)}
        onSaved={onSaved}
        toast={toast}
      />
      <CustomSetsModal
        open={stage === "custom"}
        template={template}
        onClose={() => setStage(null)}
        onSaved={onSaved}
        toast={toast}
      />
    </div>
  );
}
