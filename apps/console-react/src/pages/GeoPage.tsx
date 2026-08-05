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
import { apiGet, apiPost, apiPut } from "../api";
import { useToast } from "../hooks/useToast";

type GeoConfig = {
  enabled: boolean;
  block_countries: string[];
  allow_countries: string[];
};

const COUNTRY_PRESETS = [
  { code: "CN", name: "中国" },
  { code: "US", name: "美国" },
  { code: "RU", name: "俄罗斯" },
  { code: "KP", name: "朝鲜" },
  { code: "IR", name: "伊朗" },
];

function parseCountries(s: string): string[] {
  return s.split(",").map((x) => x.trim()).filter(Boolean);
}

export function GeoPage(): JSX.Element {
  const toast = useToast();
  const [cfg, setCfg] = useState<GeoConfig>({ enabled: false, block_countries: [], allow_countries: [] });
  const [blockStr, setBlockStr] = useState("");
  const [allowStr, setAllowStr] = useState("");
  const [testIp, setTestIp] = useState("8.8.8.8");
  const [lookupResult, setLookupResult] = useState("—");
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const res = await apiGet<{ config: GeoConfig }>("/api/geo/config");
    setCfg(res.config);
    setBlockStr(res.config.block_countries.join(","));
    setAllowStr(res.config.allow_countries.join(","));
  }, []);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const addPreset = (code: string) => {
    const parts = parseCountries(blockStr);
    if (!parts.includes(code)) parts.push(code);
    setBlockStr(parts.join(","));
  };

  const save = async () => {
    const next: GeoConfig = {
      enabled: cfg.enabled,
      block_countries: parseCountries(blockStr),
      allow_countries: parseCountries(allowStr),
    };
    try {
      await apiPut("/api/geo/config", next);
      setCfg(next);
      toast("Geo 配置已保存");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const lookup = async () => {
    try {
      const r = await apiPost<Record<string, unknown>>("/api/geo/lookup", { ip: testIp.trim() });
      setLookupResult(JSON.stringify(r, null, 2));
    } catch (e) {
      toast(String(e), false);
    }
  };

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      <NervePageHeader title="地域封禁" />
      <div className="dash-grid-2">
        <NerveCard title="Geo 策略">
          <NerveForm>
            <NerveSwitch
              label="启用地域封禁"
              checked={cfg.enabled}
              onChange={(e) => setCfg((c) => ({ ...c, enabled: e.target.checked }))}
            />
            <NerveFormItem label="封禁国家码（逗号分隔 ISO 3166-1 alpha-2）">
              <NerveInput wide value={blockStr} onChange={(e) => setBlockStr(e.target.value)} placeholder="RU,KP,IR" />
            </NerveFormItem>
            <NerveFormItem label="仅允许国家（留空表示不限制）">
              <NerveInput wide value={allowStr} onChange={(e) => setAllowStr(e.target.value)} />
            </NerveFormItem>
            <div className="chip-row">
              {COUNTRY_PRESETS.map((c) => (
                <button key={c.code} type="button" className="chip-btn" onClick={() => addPreset(c.code)}>
                  {c.name} ({c.code})
                </button>
              ))}
            </div>
            <NerveButton variant="primary" onClick={() => void save()}>保存配置</NerveButton>
          </NerveForm>
        </NerveCard>

        <NerveCard title="Geo 查询测试">
          <NerveForm>
            <NerveInput wide value={testIp} onChange={(e) => setTestIp(e.target.value)} placeholder="8.8.8.8" />
            <NerveButton onClick={() => void lookup()}>查询</NerveButton>
            <pre className="json-view">{lookupResult}</pre>
          </NerveForm>
        </NerveCard>
      </div>
    </>
  );
}
