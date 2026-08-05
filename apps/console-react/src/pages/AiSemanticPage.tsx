import { useCallback, useEffect, useState } from "react";
import { NerveButton, NervePageHeader, nerveConfirm } from "@nerve/ui";
import { apiGet, apiPost, apiPut } from "../api";
import { SemanticConsole } from "../features/semantic/SemanticConsole";
import { useToast } from "../hooks/useToast";
import { GLOBAL_DEFAULT_SITE_ID } from "../sites";
import { PAGE_META } from "../nav";
import { mergeSemanticConfig, type SemanticConfig, type SemanticStats } from "../lib/semantic";

function siteQ(siteId: string): string {
  return `?site_id=${encodeURIComponent(siteId)}`;
}

export function AiSemanticPage(): JSX.Element {
  const toast = useToast();
  const sid = GLOBAL_DEFAULT_SITE_ID;
  const [cfg, setCfg] = useState<SemanticConfig>(() => mergeSemanticConfig(undefined));
  const [stats, setStats] = useState<SemanticStats | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const [res, statsRes] = await Promise.all([
      apiGet<{ config: Partial<SemanticConfig> }>("/api/global/semantic/config"),
      apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`).catch(() => null),
    ]);
    setCfg(mergeSemanticConfig(res.config));
    setStats(statsRes);
  }, [sid]);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const save = async () => {
    const ok = await nerveConfirm({
      title: "保存语义配置",
      message: "确定保存全局默认语义配置吗？",
      okText: "保存",
    });
    if (!ok) return;
    try {
      await apiPut("/api/global/semantic/config", cfg);
      toast("全局默认语义配置已保存");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const refreshStats = async () => {
    try {
      setStats(await apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`));
      toast("观测数据已刷新");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const submitFeedback = async (payload: {
    event_id?: string;
    verdict: string;
    note?: string;
  }) => {
    const resp = await apiPost<{ config_updated?: boolean; auto_tune?: { actions?: string[] } }>(
      "/api/semantic/feedback",
      { site_id: sid, ...payload },
    );
    setStats(await apiGet<SemanticStats>(`/api/semantic/stats${siteQ(sid)}`).catch(() => stats));
    if (resp.config_updated) {
      const cfgRes = await apiGet<{ config: Partial<SemanticConfig> }>(
        "/api/global/semantic/config",
      );
      setCfg(mergeSemanticConfig(cfgRes.config));
    }
    toast(
      resp.config_updated
        ? `反馈已记录，已自动调优: ${resp.auto_tune?.actions?.join(", ") ?? ""}`
        : "反馈已记录",
    );
  };

  if (loading) return <p className="muted">加载中…</p>;

  return (
    <>
      <NervePageHeader
        title={PAGE_META["ai-semantic"].title}
        subtitle={PAGE_META["ai-semantic"].subtitle}
        actions={
          <>
            <span className="muted">全局默认</span>
            <NerveButton variant="primary" onClick={() => void save()}>
              保存配置
            </NerveButton>
          </>
        }
      />
      <SemanticConsole
        cfg={cfg}
        onChange={setCfg}
        stats={stats}
        siteId={sid}
        onRefreshStats={refreshStats}
        onSubmitFeedback={submitFeedback}
      />
    </>
  );
}
