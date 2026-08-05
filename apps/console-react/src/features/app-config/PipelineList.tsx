import { useCallback, useRef, useState } from "react";
import { NerveButton, NerveSwitch } from "@nerve/ui";
import {
  PIPELINE_STAGES,
  movePipelineStage,
  normalizePipelineEnabled,
  normalizePipelineOrder,
  type PipelineEnabledMap,
} from "../../lib/pipeline";

type Props = {
  order: string[];
  enabled: PipelineEnabledMap;
  onChange: (order: string[], enabled: PipelineEnabledMap) => void;
};

function reorder(order: string[], from: number, to: number): string[] {
  const next = [...order];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export function PipelineList({ order, enabled, onChange }: Props) {
  const normalized = normalizePipelineOrder(order);
  const en = normalizePipelineEnabled(enabled);
  const dragIndexRef = useRef<number | null>(null);
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);

  const handleDragStart = useCallback((e: React.DragEvent<HTMLDivElement>, index: number) => {
    dragIndexRef.current = index;
    setDraggingIndex(index);
    e.dataTransfer.setData("text/plain", normalized[index] ?? "");
    e.dataTransfer.effectAllowed = "move";
  }, [normalized]);

  const handleDragOver = useCallback((e: React.DragEvent<HTMLDivElement>, targetIndex: number) => {
    e.preventDefault();
    const from = dragIndexRef.current;
    if (from === null || from === targetIndex) return;

    const rect = e.currentTarget.getBoundingClientRect();
    const after = (e.clientY - rect.top) / rect.height > 0.5;
    let to = after ? targetIndex + 1 : targetIndex;
    if (from < to) to -= 1;
    if (to === from || to < 0 || to >= normalized.length) return;

    dragIndexRef.current = to;
    onChange(reorder(normalized, from, to), en);
  }, [normalized, en, onChange]);

  const handleDragEnd = useCallback(() => {
    dragIndexRef.current = null;
    setDraggingIndex(null);
  }, []);

  let exec = 0;
  return (
    <div id="app-pipeline-order" className="pipeline-order-list">
      {normalized.map((id, i) => {
        const st = PIPELINE_STAGES.find((s) => s.id === id)!;
        const on = en[id] !== false;
        if (on) exec += 1;
        return (
          <div
            key={id}
            className={`pipeline-order-row${on ? "" : " is-disabled"}${draggingIndex === i ? " dragging" : ""}`}
            data-stage={id}
            data-i={i}
            draggable
            onDragStart={(e) => handleDragStart(e, i)}
            onDragOver={(e) => handleDragOver(e, i)}
            onDragEnd={handleDragEnd}
          >
            <span className="pipeline-drag" title="拖动排序">
              ⋮⋮
            </span>
            <span className={`pipeline-num${on ? "" : " is-off"}`}>{on ? exec : "—"}</span>
            <div className="pipeline-info">
              <strong>{st.label}</strong>
              <span className="muted small">{st.desc}</span>
            </div>
            <label className="pipeline-enable-switch" title="启用此检测段">
              <NerveSwitch
                checked={on}
                onChange={(e) =>
                  onChange(normalized, { ...en, [id]: e.target.checked })
                }
              />
              <span>启用</span>
            </label>
            <code className="pipeline-id">{id}</code>
            <div className="pipeline-move">
              <NerveButton
                size="sm"
                disabled={i === 0}
                onClick={() => onChange(movePipelineStage(normalized, i, -1), en)}
              >
                ↑
              </NerveButton>
              <NerveButton
                size="sm"
                disabled={i === normalized.length - 1}
                onClick={() => onChange(movePipelineStage(normalized, i, 1), en)}
              >
                ↓
              </NerveButton>
            </div>
          </div>
        );
      })}
    </div>
  );
}
