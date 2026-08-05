import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePageHeader,
} from "@nerve/ui";
import { apiGet, apiPost } from "../api";
import { useToast } from "../hooks/useToast";

export function AiLabPage({ embedded = false }: { embedded?: boolean }): JSX.Element {
  const toast = useToast();
  const [cfg, setCfg] = useState<Record<string, unknown>>({});
  const [uri, setUri] = useState("/?id=%2527%2520OR%25201%253D1");
  const [output, setOutput] = useState("—");
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setCfg(await apiGet<Record<string, unknown>>("/api/triage/config"));
  }, []);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const inspect = async () => {
    setOutput("请求中…");
    try {
      const r = await apiPost<Record<string, unknown>>("/api/inspect", { uri, persist: false });
      setOutput(JSON.stringify(r, null, 2));
    } catch (e) {
      setOutput(String(e));
      toast(String(e), false);
    }
  };

  const explain = async () => {
    setOutput("研判中…");
    try {
      const inspect = await apiPost<{ hit?: Record<string, unknown>; triage?: Record<string, unknown>; allowed?: boolean }>(
        "/api/inspect",
        { uri, persist: false },
      );
      const event = {
        id: `lab-${Date.now()}`,
        ts: new Date().toISOString(),
        layer: "l7",
        action: inspect.hit ? "block" : "allow",
        category: String((inspect.hit as { category?: string })?.category ?? "sqli"),
        severity: String((inspect.hit as { severity?: string })?.severity ?? "medium"),
        uri,
        method: "GET",
        payload_sample: String((inspect.hit as { payload_sample?: string })?.payload_sample ?? ""),
      };
      const explainRes = await apiPost<Record<string, unknown>>("/api/triage/explain", { event });
      setOutput(
        JSON.stringify(
          { inspect: { blocked: !inspect.allowed, triage: inspect.triage }, explain: explainRes },
          null,
          2,
        ),
      );
    } catch (e) {
      setOutput(String(e));
      toast(String(e), false);
    }
  };

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      {!embedded ? <NervePageHeader title="研判实验室" /> : null}
      <NerveCard title="引擎状态">
        <ul className="muted">
          <li>
            研判引擎：<code>{String(cfg.triage_engine ?? "multi_signal_fusion_v1")}</code>
          </li>
          <li>语义检测：{cfg.semantic_enabled ? "启用" : "关闭"}</li>
          <li>
            LLM：{cfg.llm_ready ? `就绪 (${String(cfg.llm_model ?? "")})` : "template_v1 回退"}
          </li>
        </ul>
      </NerveCard>

      <NerveCard title="快速测试" style={{ marginTop: 12 }}>
        <NerveForm>
          <NerveFormItem label="测试 URI">
            <NerveInput wide value={uri} onChange={(e) => setUri(e.target.value)} />
          </NerveFormItem>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <NerveButton onClick={() => void inspect()}>inspect 检测</NerveButton>
            <NerveButton variant="primary" onClick={() => void explain()}>研判 + AI 解释</NerveButton>
          </div>
          <pre className="muted small" style={{ whiteSpace: "pre-wrap", maxHeight: 360, overflow: "auto" }}>
            {output}
          </pre>
        </NerveForm>
      </NerveCard>
    </>
  );
}
