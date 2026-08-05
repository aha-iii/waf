import { useState, useEffect } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NerveInput,
  NervePageHeader,
  NerveSelect,
  NerveTag,
} from "@nerve/ui";
import { apiPost } from "../api";
import { PolicySimRulesPicker } from "../features/policy-sim/PolicySimRulesPicker";
import { useToast } from "../hooks/useToast";
import { getSelectedSiteId, loadSites, siteDisplayName, type ProtectedSite } from "../sites";

type SimResult = {
  ok: boolean;
  error?: string;
  message?: string;
  sample_total_in_window?: number;
  sample_events?: number;
  sample_truncated?: boolean;
  sample_confidence?: string;
  total_blocks_in_sample?: number;
  impact?: {
    events_matching_disable_rules: number;
    events_matching_whitelist: number;
    blocks_relief_unique: number;
    estimated_fp_relief_pct: number;
    sample_uris: string[];
  };
  risk_level?: string;
  warnings?: string[];
  recommendations?: string[];
  note?: string;
};

const RISK_CN: Record<string, string> = {
  minimal: "极低",
  low: "低",
  medium: "中",
  high: "高",
  insufficient_data: "样本不足",
};

const CONF_CN: Record<string, string> = {
  high: "高",
  medium: "中",
  low: "低",
};

function riskTone(level?: string): "default" | "green" | "red" | "orange" | "blue" {
  if (level === "medium") return "orange";
  if (level === "low" || level === "minimal") return "blue";
  if (level === "insufficient_data") return "default";
  if (level === "high") return "red";
  return "green";
}

export function PolicySimPage({ embedded = false }: { embedded?: boolean }) {
  const toast = useToast();
  const [sites, setSites] = useState<ProtectedSite[]>([]);
  const [siteId, setSiteId] = useState("");
  const [selectedRules, setSelectedRules] = useState<string[]>([]);
  const [rulesPickerOpen, setRulesPickerOpen] = useState(false);
  const [whitelistPrefixes, setWhitelistPrefixes] = useState("");
  const [result, setResult] = useState<SimResult | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    void loadSites()
      .then((list) => {
        setSites(list);
        const preferred = getSelectedSiteId();
        const hit = list.find((s) => s.id === preferred);
        setSiteId(hit?.id ?? list[0]?.id ?? "");
      })
      .catch(() => setSites([]));
  }, []);

  const runSim = async () => {
    if (!siteId) {
      toast("请选择应用", false);
      return;
    }
    const prefixes = whitelistPrefixes
      .split(/[,，\s]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (selectedRules.length === 0 && prefixes.length === 0) {
      toast("请至少选择拟禁用规则或填写 URI 前缀", false);
      return;
    }
    for (const p of prefixes) {
      if (!p.startsWith("/")) {
        toast(`URI 前缀必须以 / 开头：${p}`, false);
        return;
      }
    }

    setLoading(true);
    try {
      const data = await apiPost<SimResult>("/api/policy/simulate", {
        site_id: siteId,
        disable_rules: selectedRules,
        whitelist_uri_prefixes: prefixes,
        sample_limit: 200,
      });
      if (!data.ok) {
        toast(data.message ?? data.error ?? "预演失败", false);
        setResult(data);
        return;
      }
      setResult(data);
      toast("预演完成（未下发策略）", true);
    } catch (e) {
      toast(String(e), false);
    } finally {
      setLoading(false);
    }
  };

  const removeRule = (id: string) => {
    setSelectedRules((prev) => prev.filter((r) => r !== id));
  };

  const formCard = (
    <NerveCard title="变更草案">
      <div className="ai-pl-form-grid">
        <label className="ai-pl-form-field">
          <span>应用</span>
          <NerveSelect
            value={siteId}
            onChange={(e) => setSiteId(e.target.value)}
            options={
              sites.length === 0
                ? [{ value: "", label: "暂无应用" }]
                : sites.map((s) => ({
                    value: s.id,
                    label: siteDisplayName(sites, s.id),
                  }))
            }
          />
        </label>
        <label className="ai-pl-form-field">
          <span>拟加白 URI 前缀</span>
          <NerveInput
            className="cloud-login-input wide"
            value={whitelistPrefixes}
            onChange={(e) => setWhitelistPrefixes(e.target.value)}
            placeholder="/api/health, /static/"
          />
          <span className="muted small">逗号分隔，须以 / 开头</span>
        </label>
        <div className="ai-pl-form-field ai-pl-form-field-wide">
          <span>拟禁用规则（最多 32 条）</span>
          <div className="ai-pl-rule-chips">
            {selectedRules.length === 0 ? (
              <span className="muted small">点击右侧按钮从规则库选择</span>
            ) : (
              selectedRules.map((id) => (
                <span key={id} className="ai-pl-rule-chip">
                  <code>{id}</code>
                  <button
                    type="button"
                    className="ai-pl-rule-chip-remove"
                    aria-label={`移除 ${id}`}
                    onClick={() => removeRule(id)}
                  >
                    ×
                  </button>
                </span>
              ))
            )}
          </div>
          <div className="ai-pl-rule-picker-row">
            <NerveButton
              size="sm"
              disabled={!siteId}
              onClick={() => {
                if (!siteId) {
                  toast("请先选择应用", false);
                  return;
                }
                setRulesPickerOpen(true);
              }}
            >
              选择规则
            </NerveButton>
            {selectedRules.length > 0 ? (
              <span className="muted small">已选 {selectedRules.length} 条</span>
            ) : null}
          </div>
        </div>
        <div className="ai-pl-form-actions">
          <NerveButton variant="primary" disabled={loading || !siteId} onClick={() => void runSim()}>
            {loading ? "预演中…" : "运行预演"}
          </NerveButton>
        </div>
      </div>
    </NerveCard>
  );

  const resultCard =
    result?.ok ? (
      <NerveCard title="预演结果">
        <div className="ai-pl-result-meta">
          <NerveTag tone={riskTone(result.risk_level)}>
            风险 {RISK_CN[result.risk_level ?? ""] ?? result.risk_level}
          </NerveTag>
          <span className="muted">
            置信度 {CONF_CN[result.sample_confidence ?? ""] ?? result.sample_confidence}
          </span>
          <span className="muted">
            样本 {result.sample_events ?? 0} 条
            {result.sample_truncated ? ` / 窗口 ${result.sample_total_in_window}` : ""}
          </span>
        </div>
        <div className="ai-pl-result-kpis">
          <div className="ai-pl-result-kpi">
            <div className="ai-pl-result-kpi-label">预计误报缓解</div>
            <div className="ai-pl-result-kpi-value">{result.impact?.estimated_fp_relief_pct ?? 0}%</div>
          </div>
          <div className="ai-pl-result-kpi">
            <div className="ai-pl-result-kpi-label">样本内拦截</div>
            <div className="ai-pl-result-kpi-value">{result.total_blocks_in_sample ?? 0}</div>
          </div>
          <div className="ai-pl-result-kpi">
            <div className="ai-pl-result-kpi-label">去重后缓解</div>
            <div className="ai-pl-result-kpi-value">{result.impact?.blocks_relief_unique ?? 0}</div>
          </div>
        </div>
        <div className="ai-pl-result-kpis">
          <div className="ai-pl-result-kpi">
            <div className="ai-pl-result-kpi-label">匹配禁用规则</div>
            <div className="ai-pl-result-kpi-value" style={{ fontSize: 18 }}>
              {result.impact?.events_matching_disable_rules ?? 0}
            </div>
          </div>
          <div className="ai-pl-result-kpi">
            <div className="ai-pl-result-kpi-label">匹配 URI 加白</div>
            <div className="ai-pl-result-kpi-value" style={{ fontSize: 18 }}>
              {result.impact?.events_matching_whitelist ?? 0}
            </div>
          </div>
        </div>
        {result.warnings && result.warnings.length > 0 ? (
          <div className="ai-pl-result-section">
            <h4>注意</h4>
            <ul>
              {result.warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          </div>
        ) : null}
        {result.recommendations && result.recommendations.length > 0 ? (
          <div className="ai-pl-result-section">
            <h4>建议</h4>
            <ul>
              {result.recommendations.map((r) => (
                <li key={r}>{r}</li>
              ))}
            </ul>
          </div>
        ) : null}
        {result.impact?.sample_uris && result.impact.sample_uris.length > 0 ? (
          <p className="muted small" style={{ marginTop: 12 }}>
            样例 URI：{result.impact.sample_uris.join(" · ")}
          </p>
        ) : null}
        {result.note ? (
          <p className="muted small" style={{ marginTop: 8 }}>
            {result.note}
          </p>
        ) : null}
      </NerveCard>
    ) : null;

  return (
    <div className="ai-pl-page">
      {!embedded ? (
        <NervePageHeader title="策略预演" subtitle="基于近 7 天事件样本估算变更影响，不下发" />
      ) : null}

      <NerveAlert tone="info">
        预演仅作决策参考；正式禁用规则或加白需人工确认后应用。
      </NerveAlert>

      {result && !result.ok ? (
        <NerveAlert tone="error">{result.message ?? result.error ?? "请求被拒绝"}</NerveAlert>
      ) : null}

      {result?.ok ? (
        <div className="ai-pl-layout-split">
          {formCard}
          {resultCard}
        </div>
      ) : (
        formCard
      )}

      <PolicySimRulesPicker
        open={rulesPickerOpen}
        siteId={siteId}
        selected={selectedRules}
        onClose={() => setRulesPickerOpen(false)}
        onConfirm={(ids) => {
          setSelectedRules(ids);
          setRulesPickerOpen(false);
        }}
      />
    </div>
  );
}
