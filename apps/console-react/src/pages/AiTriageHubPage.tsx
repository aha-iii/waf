import { useMemo, useState } from "react";
import { NervePageHeader, NerveTabBar } from "@nerve/ui";
import { AiLabPage } from "./AiLabPage";
import { AiLlmPage } from "./AiLlmPage";

type TabId = "lab" | "llm";

function initialTab(): TabId {
  const raw = (location.hash.replace(/^#\/?/, "") || "").split("?")[0];
  if (raw === "ai-llm") return "llm";
  return "lab";
}

export function AiTriageHubPage() {
  const [tab, setTab] = useState<TabId>(initialTab);
  const tabs = useMemo(
    () => [
      { id: "lab", label: "实验室" },
      { id: "llm", label: "LLM 配置" },
    ],
    [],
  );

  return (
    <>
      <NervePageHeader
        title="智能研判"
        subtitle="单条 URI 检测、研判解释与 LLM 连接配置"
      />
      <NerveTabBar tabs={tabs} activeId={tab} onChange={(id) => setTab(id as TabId)} />
      <div style={{ marginTop: 16 }}>
        {tab === "lab" ? <AiLabPage embedded /> : <AiLlmPage embedded />}
      </div>
    </>
  );
}
