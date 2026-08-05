import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveEmpty,
  NerveModal,
  NervePagination,
  NerveSearchInput,
  NerveSwitch,
  defaultPaginationState,
  paginate,
  type NervePaginationState,
} from "@nerve/ui";
import { categoryLabel } from "../../lib/labels";
import {
  RISK_LABEL,
  PROTECT_LABEL,
  fetchRuleRows,
  normalizeCategory,
  ruleAtProtectLevel,
  ruleHintById,
  rowsForCategory,
  type ModuleConfig,
  type ProtectLevel,
  type RiskTier,
  type RuleRow,
} from "../../lib/crs-rules";
import {
  crsRuleTogglesLimited,
  fetchCrsRuntime,
  type CrsRuntimeStatus,
} from "../../lib/crs-runtime";

export type ModuleRulesModalProps = {
  open: boolean;
  onClose: () => void;
  category: string;
  protect: ProtectLevel;
  moduleTitle: string;
  moduleEnabled?: boolean;
  moduleKey?: string;
  siteId?: string;
  /** 应用抽屉内可编辑本应用规则集 */
  editable?: boolean;
  disabledRules?: string[];
  onSave?: (disabledRules: string[]) => void;
  /** 打开后定位并高亮指定规则 */
  highlightRuleId?: string;
  /** 专家防护运行时；未传入时在弹窗打开时拉取 */
  crsRuntime?: CrsRuntimeStatus;
};

const RISK_ORDER: RiskTier[] = ["high", "medium", "low"];

function RuleTierTable(props: {
  rules: RuleRow[];
  protect: ProtectLevel;
  editable: boolean;
  disabled: Set<string>;
  snapshotOff: Set<string>;
  highlightRuleId?: string;
  onToggle?: (id: string, on: boolean) => void;
}) {
  const { rules, protect, editable, disabled, snapshotOff, highlightRuleId, onToggle } = props;

  return (
    <div className="rule-table-wrap">
      <table
        className={`data-table rule-detail-table${editable ? " rule-detail-table-editable" : ""}`}
      >
        <thead>
          <tr>
            <th className="col-id">ID</th>
            <th className="col-name">名称</th>
            <th className="col-risk">危级</th>
            <th className="col-pl">PL</th>
            {editable ? <th className="col-toggle">启用</th> : null}
            <th className="col-status">状态</th>
            <th className="col-desc">检测说明</th>
            <th className="col-example">攻击示例</th>
          </tr>
        </thead>
        <tbody>
          {rules.map((r) => {
            const atLevel = ruleAtProtectLevel(r, protect);
            const ruleOff = editable ? disabled.has(r.id) : snapshotOff.has(r.id);
            const appRuleOn = atLevel && !ruleOff;
            const hint = ruleHintById(r.id, r.name, r.category);
            const pl = Math.max(1, Math.min(4, r.paranoia_level || 1));
            const isHighlight = highlightRuleId?.trim() === r.id;

            return (
              <tr
                key={r.id}
                className={[appRuleOn ? "" : "muted", isHighlight ? "rule-row-highlight" : ""]
                  .filter(Boolean)
                  .join(" ") || undefined}
                data-mod-rule-id={r.id}
              >
                <td className="col-id">
                  <code>{r.id}</code>
                </td>
                <td className="col-name">{r.name}</td>
                <td className="col-risk">
                  <span className={`risk-tag risk-tag-${r.risk}`}>{RISK_LABEL[r.risk]}</span>
                </td>
                <td className="col-pl">
                  <span className={`pl-tag pl-tag-${pl}`} title={`Paranoia Level ${pl}`}>
                    PL{pl}
                  </span>
                </td>
                {editable ? (
                  <td className="col-toggle">
                    <NerveSwitch
                      sm
                      checked={appRuleOn}
                      disabled={!atLevel}
                      title={
                        atLevel
                          ? "本应用启用/关闭此规则"
                          : "当前灵敏度未纳入，请提高模块灵敏度"
                      }
                      onChange={(e) => onToggle?.(r.id, e.target.checked)}
                    />
                  </td>
                ) : null}
                <td className="col-status">
                  {ruleOff ? (
                    <span className="risk-tag risk-tag-off">
                      {editable ? "已关闭" : "本应用关闭"}
                    </span>
                  ) : !atLevel ? (
                    <span className="risk-tag risk-tag-off">未纳入</span>
                  ) : (
                    <span className="risk-tag risk-tag-block">拦截</span>
                  )}
                </td>
                <td className="col-desc rule-desc-col">
                  <span className="rule-hint-desc">{hint.desc}</span>
                </td>
                <td className="col-example rule-example-col">
                  {hint.example ? (
                    <code className="rule-hint-ex">{hint.example}</code>
                  ) : (
                    <span className="muted rule-hint-none">—</span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function ModuleRulesModal(props: ModuleRulesModalProps) {
  const {
    open,
    onClose,
    category,
    protect,
    moduleTitle,
    moduleEnabled = true,
    moduleKey,
    siteId,
    editable: editableProp = false,
    disabledRules: initialDisabled,
    onSave,
    highlightRuleId,
    crsRuntime: crsRuntimeProp,
  } = props;

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [allRows, setAllRows] = useState<RuleRow[]>([]);
  const [moduleConfigs, setModuleConfigs] = useState<Record<string, ModuleConfig>>({});
  const [fetchedCrsRuntime, setFetchedCrsRuntime] = useState<CrsRuntimeStatus | null>(null);
  const [filter, setFilter] = useState("");
  const [disabled, setDisabled] = useState<Set<string>>(() => new Set(initialDisabled ?? []));
  const [pageByRisk, setPageByRisk] = useState<Record<RiskTier, NervePaginationState>>({
    high: defaultPaginationState(),
    medium: defaultPaginationState(),
    low: defaultPaginationState(),
  });

  const crsRuntime = crsRuntimeProp ?? fetchedCrsRuntime ?? undefined;
  const wasmRuleLimited = crsRuntime ? crsRuleTogglesLimited(crsRuntime) : false;
  const editable = editableProp && !wasmRuleLimited;
  const titleSuffix = editable ? " · 本应用规则集" : " · 规则预览";

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchRuleRows(siteId);
      setAllRows(data.allRows);
      setModuleConfigs(data.moduleConfigs);
      if (!data.allRows.some((r) => normalizeCategory(r.category) === category)) {
        setError(`${categoryLabel(category)} 暂无规则`);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [siteId, category]);

  useEffect(() => {
    if (!open) return;
    setFilter(highlightRuleId?.trim() ?? "");
    setDisabled(new Set(initialDisabled ?? []));
    setFetchedCrsRuntime(null);
    setPageByRisk({
      high: defaultPaginationState(),
      medium: defaultPaginationState(),
      low: defaultPaginationState(),
    });
    void load();
    if (!crsRuntimeProp) {
      void fetchCrsRuntime()
        .then(setFetchedCrsRuntime)
        .catch(() => {
          /* 与原版一致：无 crsRuntime 时不限制开关 */
        });
    }
  }, [open, load, initialDisabled, highlightRuleId, crsRuntimeProp]);

  useEffect(() => {
    if (!open) return;
    let modalEl: Element | null = null;
    const applyXxl = () => {
      const modals = document.querySelectorAll(".ant-modal");
      modalEl = modals[modals.length - 1] ?? null;
      modalEl?.classList.add("ant-modal-xxl");
    };
    const id = window.requestAnimationFrame(applyXxl);
    return () => {
      window.cancelAnimationFrame(id);
      modalEl?.classList.remove("ant-modal-xxl");
    };
  }, [open]);

  useEffect(() => {
    if (!open || !highlightRuleId || loading) return;
    const hid = highlightRuleId.trim();
    const t = window.setTimeout(() => {
      const row = document.querySelector<HTMLElement>(`tr[data-mod-rule-id="${CSS.escape(hid)}"]`);
      row?.classList.add("rule-row-highlight");
      row?.scrollIntoView({ block: "center", behavior: "smooth" });
    }, 80);
    return () => window.clearTimeout(t);
  }, [open, highlightRuleId, loading, filter, allRows]);

  const catRows = useMemo(() => {
    return rowsForCategory(allRows, category, filter).filter((r) => ruleAtProtectLevel(r, protect));
  }, [allRows, category, filter, protect]);

  const byRisk = useMemo(() => {
    const map: Record<RiskTier, RuleRow[]> = { high: [], medium: [], low: [] };
    for (const r of catRows) map[r.risk].push(r);
    return map;
  }, [catRows]);

  const modSnapshot = moduleKey
    ? moduleConfigs[moduleKey]
    : (moduleConfigs[category] ?? { enabled: true, action: "block", sensitivity: "medium" });
  const snapshotOff = new Set(modSnapshot?.disabled_rules ?? []);

  const toggleRule = (id: string, on: boolean) => {
    setDisabled((prev) => {
      const next = new Set(prev);
      if (on) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleSave = () => {
    onSave?.([...disabled].sort());
    onClose();
  };

  const footer = editable ? (
    <div>
      <NerveButton onClick={onClose}>取消</NerveButton>
      <NerveButton variant="primary" onClick={handleSave}>
        确定
      </NerveButton>
    </div>
  ) : undefined;

  return (
    <NerveModal
      open={open}
      title={`${moduleTitle}${titleSuffix}`}
      onClose={onClose}
      xl
      footer={footer}
    >
        {loading ? <div className="page-loading">加载规则中…</div> : null}
        {error && !loading ? <NerveAlert tone="error">{error}</NerveAlert> : null}

        {!loading && !error ? (
          <>
            {wasmRuleLimited ? (
              <NerveAlert tone="warning" style={{ marginBottom: 12 }}>
                专家防护启用时，单规则开关不作用于专家段。
              </NerveAlert>
            ) : null}

            {!moduleEnabled ? (
              <NerveAlert tone="warning" style={{ marginBottom: 12 }}>
                模块已关闭
              </NerveAlert>
            ) : null}

            <div className="protect-level-bar" style={{ marginBottom: 12 }}>
              <span className="protect-level-label">拦截等级</span>
              <span className="protect-level-tab active">{PROTECT_LABEL[protect]}</span>
            </div>

            <div className="table-toolbar" style={{ marginBottom: 8 }}>
              <NerveSearchInput
                placeholder="搜索 ID / 名称"
                value={filter}
                style={{ width: 220 }}
                onChange={(e) => {
                  setFilter(e.target.value);
                  setPageByRisk({
                    high: defaultPaginationState(),
                    medium: defaultPaginationState(),
                    low: defaultPaginationState(),
                  });
                }}
              />
              {editable && disabled.size > 0 ? (
                <span className="muted small" style={{ marginLeft: 12 }}>
                  已关闭 {disabled.size} 条
                </span>
              ) : null}
            </div>

            {catRows.length === 0 ? <NerveEmpty description="暂无规则" /> : null}

            <div className="mod-rules-sections">
              {RISK_ORDER.map((risk) => {
                const rules = byRisk[risk];
                if (!rules.length) return null;
                const st = pageByRisk[risk];
                const pageItems = paginate(rules, st.page, st.pageSize);
                return (
                  <div key={risk} className={`tier-block tier-${risk}`} style={{ marginBottom: 16 }}>
                    <h4>
                      <span className={`risk-section-tag risk-tag-${risk}`}>{RISK_LABEL[risk]}</span>{" "}
                      <span className="muted">({rules.length})</span>
                    </h4>
                    <RuleTierTable
                      rules={pageItems}
                      protect={protect}
                      editable={editable}
                      disabled={disabled}
                      snapshotOff={snapshotOff}
                      highlightRuleId={highlightRuleId}
                      onToggle={toggleRule}
                    />
                    <NervePagination
                      total={rules.length}
                      {...st}
                      onChange={(next) => setPageByRisk((prev) => ({ ...prev, [risk]: next }))}
                    />
                  </div>
                );
              })}
            </div>
          </>
        ) : null}
    </NerveModal>
  );
}
