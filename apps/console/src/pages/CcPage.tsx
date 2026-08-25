/** Console page: CC / rate-limit protection settings. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePageHeader,
  NerveSwitch,
} from "@nerve/ui";
import { apiGet, apiPut } from "../api";
import { useToast } from "../hooks/useToast";

type CcConfig = {
  enabled: boolean;
  requests_per_minute: number;
  burst: number;
  block_duration_sec: number;
};

export function CcPage(): JSX.Element {
  const toast = useToast();
  const [cfg, setCfg] = useState<CcConfig>({
    enabled: false,
    requests_per_minute: 60,
    burst: 10,
    block_duration_sec: 300,
  });
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const res = await apiGet<{ config: CcConfig }>("/api/cc/config");
    setCfg(res.config);
  }, []);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const save = async () => {
    try {
      await apiPut("/api/cc/config", cfg);
      toast("CC 配置已保存");
    } catch (e) {
      toast(String(e), false);
    }
  };

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      <NervePageHeader title="CC 防护" subtitle="按 IP + Host + URI 令牌桶限流" />
      <NerveCard title="CC 限流配置">
        <NerveForm>
          <NerveSwitch
            label="启用 CC 防护"
            checked={cfg.enabled}
            onChange={(e) => setCfg((c) => ({ ...c, enabled: e.target.checked }))}
          />
          <NerveFormItem label="每分钟请求数">
            <NerveInput
              type="number"
              value={String(cfg.requests_per_minute)}
              onChange={(e) => setCfg((c) => ({ ...c, requests_per_minute: Number(e.target.value) }))}
            />
          </NerveFormItem>
          <NerveFormItem label="突发容量">
            <NerveInput
              type="number"
              value={String(cfg.burst)}
              onChange={(e) => setCfg((c) => ({ ...c, burst: Number(e.target.value) }))}
            />
          </NerveFormItem>
          <NerveFormItem label="封禁时长（秒）">
            <NerveInput
              type="number"
              value={String(cfg.block_duration_sec)}
              onChange={(e) => setCfg((c) => ({ ...c, block_duration_sec: Number(e.target.value) }))}
            />
          </NerveFormItem>
          <NerveButton variant="primary" onClick={() => void save()}>保存</NerveButton>
        </NerveForm>
      </NerveCard>
    </>
  );
}
