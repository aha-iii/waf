/** Console page: policy change rehearsal and false-positive samples. */
import { Suspense, lazy, useMemo, useState } from "react";
import { NervePageHeader, NerveTabBar } from "@nerve/ui";
import { AiTrainingPage } from "./AiTrainingPage";
import { PolicySimPage } from "./PolicySimPage";

const AiOpsPanel = lazy(() =>
  import("./AiOpsPanel").then((m) => ({ default: m.AiOpsPanel })),
);

type TabId = "sim" | "samples" | "ops";

function initialTab(): TabId {
  const raw = (location.hash.replace(/^#\/?/, "") || "").split("?")[0];
  if (raw === "ai-training") return "samples";
  return "sim";
}

export function AiPolicyLearningPage() {
  const [tab, setTab] = useState<TabId>(initialTab);
  const tabs = useMemo(
    () => [
      { id: "sim", label: "策略预演" },
      { id: "samples", label: "样本库" },
      { id: "ops", label: "运行状态" },
    ],
    [],
  );

  return (
    <>
      <NervePageHeader title="策略与学习" subtitle="变更预演与误报样本管理" />
      <NerveTabBar tabs={tabs} activeId={tab} onChange={(id) => setTab(id as TabId)} />
      <div className="ai-pl-page" style={{ marginTop: 16 }}>
        <Suspense fallback={<p className="muted">加载中…</p>}>
          {tab === "sim" ? (
            <PolicySimPage embedded />
          ) : tab === "samples" ? (
            <AiTrainingPage embedded />
          ) : (
            <AiOpsPanel />
          )}
        </Suspense>
      </div>
    </>
  );
}
