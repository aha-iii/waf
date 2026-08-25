/** Console page: bot management thresholds and actions. */
import { useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePageHeader,
} from "@nerve/ui";
import { apiPost } from "../api";
import { useToast } from "../hooks/useToast";

const BOT_SAMPLES = [
  { ua: "Mozilla/5.0 (compatible; Googlebot/2.1)", uri: "/robots.txt", label: "Googlebot" },
  { ua: "curl/7.68.0", uri: "/admin", label: "curl 扫描" },
  { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0", uri: "/", label: "正常浏览器" },
  { ua: "python-requests/2.28.0", uri: "/api/login", label: "脚本客户端" },
];

type BotScore = { score: number; is_bot: boolean; reason: string };

function BotScoreBox({ result }: { result: BotScore | null }) {
  if (!result) return <div className="bot-result-box">—</div>;
  return (
    <div className={`bot-score ${result.is_bot ? "bot" : "human"}`}>
      <div className="bot-score-val">{result.score}</div>
      <div>
        <strong>{result.is_bot ? "Bot / 自动化" : "疑似人类"}</strong>
        <br />
        <span className="muted">{result.reason}</span>
      </div>
    </div>
  );
}

export function BotPage(): JSX.Element {
  const toast = useToast();
  const [ua, setUa] = useState("");
  const [uri, setUri] = useState("/");
  const [result, setResult] = useState<BotScore | null>(null);
  const [sampleResults, setSampleResults] = useState<Record<string, BotScore | null>>({});

  const runScore = async (userAgent: string, requestUri: string): Promise<BotScore> => {
    return apiPost<BotScore>("/api/bot/score", { user_agent: userAgent, uri: requestUri });
  };

  const handleScore = async () => {
    try {
      setResult(await runScore(ua, uri));
    } catch (e) {
      toast(String(e), false);
    }
  };

  const handleSample = async (key: string, sampleUa: string, sampleUri: string) => {
    try {
      const r = await runScore(sampleUa, sampleUri);
      setSampleResults((prev) => ({ ...prev, [key]: r }));
    } catch (e) {
      toast(String(e), false);
    }
  };

  return (
    <>
      <NervePageHeader title="Bot 管理" />
      <div className="dash-grid-2" style={{ marginTop: 16 }}>
        <NerveCard title="自定义检测">
          <NerveForm>
            <NerveFormItem label="User-Agent">
              <NerveInput wide value={ua} onChange={(e) => setUa(e.target.value)} placeholder="Mozilla/5.0 ..." />
            </NerveFormItem>
            <NerveFormItem label="URI">
              <NerveInput wide value={uri} onChange={(e) => setUri(e.target.value)} />
            </NerveFormItem>
            <NerveButton variant="primary" onClick={() => void handleScore()}>评分</NerveButton>
            <BotScoreBox result={result} />
          </NerveForm>
        </NerveCard>

        <NerveCard title="样本测试">
          <div className="sample-list">
            {BOT_SAMPLES.map((s) => (
              <div key={s.label} className="sample-row">
                <div>
                  <strong>{s.label}</strong>
                  <div className="muted small">{s.ua.slice(0, 50)}…</div>
                </div>
                <NerveButton size="sm" onClick={() => void handleSample(s.label, s.ua, s.uri)}>测试</NerveButton>
                <div className="sample-out">
                  {sampleResults[s.label] ? <BotScoreBox result={sampleResults[s.label]!} /> : null}
                </div>
              </div>
            ))}
          </div>
        </NerveCard>
      </div>
    </>
  );
}
