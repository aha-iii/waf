/** policy-sim feature: Policy Sim Rules Picker. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCheckbox,
  NerveEmpty,
  NerveModal,
  NervePagination,
  NerveSearchInput,
  NerveSelect,
  defaultPaginationState,
  paginate,
} from "@nerve/ui";
import { categoryLabel } from "../../lib/labels";
import {
  fetchRuleRows,
  normalizeCategory,
  ruleHintById,
  type RuleRow,
} from "../../lib/crs-rules";

const MAX_RULES = 32;

export type PolicySimRulesPickerProps = {
  open: boolean;
  siteId: string;
  selected: string[];
  onClose: () => void;
  onConfirm: (ruleIds: string[]) => void;
};

export function PolicySimRulesPicker({
  open,
  siteId,
  selected,
  onClose,
  onConfirm,
}: PolicySimRulesPickerProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [allRows, setAllRows] = useState<RuleRow[]>([]);
  const [picked, setPicked] = useState<Set<string>>(() => new Set(selected));
  const [filter, setFilter] = useState("");
  const [category, setCategory] = useState("");
  const [page, setPage] = useState(defaultPaginationState());

  const load = useCallback(async () => {
    if (!siteId) return;
    setLoading(true);
    setError(null);
    try {
      const data = await fetchRuleRows(siteId);
      setAllRows(data.allRows);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [siteId]);

  useEffect(() => {
    if (!open) return;
    setPicked(new Set(selected));
    setFilter("");
    setCategory("");
    setPage(defaultPaginationState());
    void load();
  }, [open, load, selected]);

  const categories = useMemo(() => {
    const set = new Set<string>();
    for (const r of allRows) set.add(normalizeCategory(r.category));
    return [...set].sort((a, b) => categoryLabel(a).localeCompare(categoryLabel(b), "zh"));
  }, [allRows]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return allRows.filter((r) => {
      if (category && normalizeCategory(r.category) !== category) return false;
      if (!q) return true;
      const hint = ruleHintById(r.id, r.name, r.category);
      return (
        r.id.toLowerCase().includes(q) ||
        r.name.toLowerCase().includes(q) ||
        hint.desc.toLowerCase().includes(q)
      );
    });
  }, [allRows, category, filter]);

  const pageRows = paginate(filtered, page.page, page.pageSize);

  const toggle = (id: string, checked: boolean) => {
    setPicked((prev) => {
      const next = new Set(prev);
      if (checked) {
        if (next.size >= MAX_RULES) return prev;
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  };

  const atMax = picked.size >= MAX_RULES;

  return (
    <NerveModal
      open={open}
      title="选择拟禁用规则"
      xl
      onClose={onClose}
      footer={
        <div>
          <span className="muted small" style={{ marginRight: 12 }}>
            已选 {picked.size} / {MAX_RULES}
          </span>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" onClick={() => onConfirm([...picked].sort())}>
            确定
          </NerveButton>
        </div>
      }
    >
      {loading ? <p className="muted">加载规则中…</p> : null}
      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
      {!loading && !error ? (
        <>
          <div className="table-toolbar" style={{ marginBottom: 12, gap: 8, display: "flex", flexWrap: "wrap" }}>
            <NerveSearchInput
              placeholder="搜索规则 ID / 名称"
              value={filter}
              style={{ width: 220 }}
              onChange={(e) => {
                setFilter(e.target.value);
                setPage(defaultPaginationState());
              }}
            />
            <NerveSelect
              value={category}
              style={{ width: 160 }}
              options={[
                { value: "", label: "全部分类" },
                ...categories.map((c) => ({ value: c, label: categoryLabel(c) })),
              ]}
              onChange={(e) => {
                setCategory(e.target.value);
                setPage(defaultPaginationState());
              }}
            />
          </div>
          {atMax ? (
            <NerveAlert tone="warning" style={{ marginBottom: 12 }}>
              最多选择 {MAX_RULES} 条规则
            </NerveAlert>
          ) : null}
          {filtered.length === 0 ? (
            <NerveEmpty description="暂无匹配规则" />
          ) : (
            <>
              <div className="rule-table-wrap">
                <table className="data-table rule-detail-table">
                  <thead>
                    <tr>
                      <th style={{ width: 48 }} />
                      <th className="col-id">ID</th>
                      <th className="col-name">名称</th>
                      <th className="col-desc">分类</th>
                      <th className="col-desc">检测说明</th>
                    </tr>
                  </thead>
                  <tbody>
                    {pageRows.map((r) => {
                      const checked = picked.has(r.id);
                      const hint = ruleHintById(r.id, r.name, r.category);
                      return (
                        <tr key={r.id} className={checked ? "rule-row-highlight" : undefined}>
                          <td>
                            <NerveCheckbox
                              checked={checked}
                              disabled={!checked && atMax}
                              aria-label={`选择规则 ${r.id}`}
                              onChange={(e) => toggle(r.id, e.target.checked)}
                            />
                          </td>
                          <td className="col-id">
                            <code>{r.id}</code>
                          </td>
                          <td className="col-name">{r.name}</td>
                          <td>{categoryLabel(normalizeCategory(r.category))}</td>
                          <td className="col-desc">
                            <span className="rule-hint-desc">{hint.desc}</span>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
              <NervePagination total={filtered.length} {...page} onChange={setPage} />
            </>
          )}
        </>
      ) : null}
    </NerveModal>
  );
}
