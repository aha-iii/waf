/** app-config feature: Pipeline Flow Editor. */
import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import {
  bindPipelineFlowEditor,
  destroyPipelineFlowEditor,
  mergePipelineEditor,
  readPipelineFlowState,
  type PipelineEditorState,
} from "../../lib/pipeline-flow";
import type { PipelineEnabledMap } from "../../lib/pipeline";

export type PipelineFlowEditorHandle = {
  readState: () => ReturnType<typeof readPipelineFlowState>;
};

type Props = {
  order: string[];
  editor: PipelineEditorState;
  enabled: PipelineEnabledMap;
  onChange?: () => void;
};

export const PipelineFlowEditor = forwardRef<PipelineFlowEditorHandle, Props>(
  function PipelineFlowEditor({ order, editor, enabled, onChange }, ref) {
    const shellRef = useRef<HTMLDivElement>(null);
    const onChangeRef = useRef(onChange);
    onChangeRef.current = onChange;

    useImperativeHandle(ref, () => ({
      readState: () => readPipelineFlowState(shellRef.current),
    }));

    useEffect(() => {
      const el = shellRef.current;
      if (!el) return;
      bindPipelineFlowEditor(
        el,
        order,
        mergePipelineEditor(editor),
        enabled,
        () => onChangeRef.current?.(),
      );
      return () => destroyPipelineFlowEditor(el);
      // Bind only on mount/unmount; orchestration state is maintained inside LogicFlow; readState on save or mode switch
    }, []);

    return (
      <div className="pipeline-lf-shell" id="pipeline-lf-shell" ref={shellRef}>
        <div className="pipeline-lf-toolbar">
          <span className="pipeline-lf-brand">LogicFlow 编排</span>
          <button type="button" className="ant-btn ant-btn-sm" id="flow-fullscreen">
            全屏
          </button>
          <button type="button" className="ant-btn ant-btn-sm" id="flow-auto-layout">
            自动布局
          </button>
          <button type="button" className="ant-btn ant-btn-sm" id="flow-fit-view">
            适应画布
          </button>
        </div>
        <div className="pipeline-lf-host" id="app-pipeline-lf-host" />
      </div>
    );
  },
);
