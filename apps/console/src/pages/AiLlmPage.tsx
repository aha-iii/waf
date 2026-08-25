/** Console page: LLM provider and model settings for triage. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePageHeader,
  NerveSwitch,
  NerveTag,
} from "@nerve/ui";
import { apiGet, apiPut } from "../api";
import { useToast } from "../hooks/useToast";

type LlmConfigView = {
  enabled: boolean;
  auto_explain: boolean;
  api_url: string;
  model: string;
  timeout_sec: number;
  api_key_set: boolean;
  llm_ready: boolean;
};

export function AiLlmPage({ embedded = false }: { embedded?: boolean }): JSX.Element {
  const toast = useToast();
  const [cfg, setCfg] = useState<LlmConfigView>({
    enabled: false,
    auto_explain: false,
    api_url: "",
    model: "",
    timeout_sec: 30,
    api_key_set: false,
    llm_ready: false,
  });
  const [apiKey, setApiKey] = useState("");
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const res = await apiGet<{ config: LlmConfigView }>("/api/triage/llm-config");
    setCfg(res.config);
    setApiKey("");
  }, []);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const save = async () => {
    const body: Record<string, unknown> = {
      enabled: cfg.enabled,
      auto_explain: cfg.auto_explain,
      api_url: cfg.api_url.trim(),
      model: cfg.model.trim(),
      timeout_sec: Math.min(120, Math.max(5, cfg.timeout_sec || 30)),
    };
    const key = apiKey.trim();
    if (key) body.api_key = key;
    try {
      await apiPut("/api/triage/llm-config", body);
      toast("LLM 配置已保存");
      await load();
    } catch (e) {
      toast(String(e), false);
    }
  };

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      {!embedded ? <NervePageHeader title="LLM 解释" /> : null}
      <NerveCard title="LLM 连接">
        <NerveForm>
          <NerveSwitch
            label="启用 LLM 解释"
            checked={cfg.enabled}
            onChange={(e) => setCfg((c) => ({ ...c, enabled: e.target.checked }))}
          />
          <NerveSwitch
            label="inspect 命中后异步预生成 ai_summary"
            checked={cfg.auto_explain}
            onChange={(e) => setCfg((c) => ({ ...c, auto_explain: e.target.checked }))}
          />
          <NerveFormItem label="API URL">
            <NerveInput wide value={cfg.api_url} onChange={(e) => setCfg((c) => ({ ...c, api_url: e.target.value }))} />
          </NerveFormItem>
          <NerveFormItem label="模型">
            <NerveInput wide value={cfg.model} onChange={(e) => setCfg((c) => ({ ...c, model: e.target.value }))} />
          </NerveFormItem>
          <NerveFormItem label="HTTP 超时（秒，5–120）">
            <NerveInput
              wide
              type="number"
              min={5}
              max={120}
              value={String(cfg.timeout_sec)}
              onChange={(e) =>
                setCfg((c) => ({
                  ...c,
                  timeout_sec: Number.parseInt(e.target.value, 10) || 30,
                }))
              }
            />
          </NerveFormItem>
          <NerveFormItem
            label={
              <>
                API Key{" "}
                {cfg.api_key_set ? <NerveTag tone="green">已配置</NerveTag> : <span className="muted">未配置</span>}
              </>
            }
          >
            <NerveInput
              wide
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={cfg.api_key_set ? "留空保持不变" : "sk-..."}
              autoComplete="off"
            />
          </NerveFormItem>
          <p className="muted small">{cfg.llm_ready ? "LLM 就绪" : "未就绪"}</p>
          <NerveButton variant="primary" onClick={() => void save()}>保存 LLM 配置</NerveButton>
        </NerveForm>
      </NerveCard>
    </>
  );
}
