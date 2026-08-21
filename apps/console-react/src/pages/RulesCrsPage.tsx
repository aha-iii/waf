/** Console page: CRS / core rule library browse and per-app toggles. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveCard,
  NervePageHeader,
  NervePagination,
  NervePanelCard,
  NerveSearchInput,
  NerveSyncActions,
  NerveTabBar,
  NerveTable,
  NerveTableActions,
  NERVE_COL_ACTIONS,
  NerveTag,
  defaultPaginationState,
  nerveConfirm,
  nerveResult,
  paginate,
} from "@nerve/ui";
import { apiGet, apiPost } from "../api";
import { useToast } from "../hooks/useToast";
import {
  CATEGORY_ORDER,
  fetchRuleRows,
  normalizeCategory,
  PROTECT_LABEL,
  rowsForCategory,
  ruleAtProtectLevel,
  sensitivityToProtect,
  type CrsVersion,
  type ModuleConfig,
  type PackSummary,
  type ProtectLevel,
  type RuleRow,
} from "../lib/crs-rules";
import { categoryLabel } from "../lib/labels";

export function RulesCrsPage() {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [allRows, setAllRows] = useState<RuleRow[]>([]);
  const [moduleConfigs, setModuleConfigs] = useState<Record<string, ModuleConfig>>({});
  const [packs, setPacks] = useState<PackSummary[]>([]);
  const [versions, setVersions] = useState<CrsVersion[]>([]);
  const [activeCategory, setActiveCategory] = useState("sqli");
  const [activeProtect, setActiveProtect] = useState<ProtectLevel>(1);
  const [filter, setFilter] = useState("");
  const [updating, setUpdating] = useState(false);
  const [packsPage, setPacksPage] = useState(defaultPaginationState());
  const [versionsPage, setVersionsPage] = useState(defaultPaginationState());
  const [rulesPage, setRulesPage] = useState(defaultPaginationState());

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [inv, ver, data] = await Promise.all([
        apiGet<{ packs: PackSummary[]; rules_total: number }>("/api/rules/packs?limit=2000"),
        apiGet<{ versions: CrsVersion[]; max_versions: number }>("/api/rules/crs/versions"),
        fetchRuleRows(),
      ]);
      setPacks(inv.packs ?? []);
      setVersions(ver.versions ?? []);
      setAllRows(data.allRows);
      setModuleConfigs(data.moduleConfigs);
      setActiveCategory((prev) => {
        if (data.allRows.some((r) => normalizeCategory(r.category) === prev)) return prev;
        return (
          CATEGORY_ORDER.find((c) => data.allRows.some((r) => normalizeCategory(r.category) === c)) ??
          "sqli"
        );
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Tab switch only updates local filters and intercept level; no refetch or full-page loading
  useEffect(() => {
    const mod = moduleConfigs[activeCategory];
    if (mod) setActiveProtect(sensitivityToProtect(mod.sensitivity));
    setRulesPage((p) => ({ ...p, page: 1 }));
  }, [activeCategory, moduleConfigs]);

  const stats = useMemo(() => {
    const activeN = allRows.filter((r) => r.active).length;
    const highN = allRows.filter((r) => r.risk === "high").length;
    const medN = allRows.filter((r) => r.risk === "medium").length;
    const lowN = allRows.filter((r) => r.risk === "low").length;
    return { activeN, highN, medN, lowN };
  }, [allRows]);

  const categoryTabs = useMemo(() => {
    const counts = new Map<string, number>();
    for (const r of allRows) {
      const c = normalizeCategory(r.category);
      counts.set(c, (counts.get(c) ?? 0) + 1);
    }
    const ordered: string[] = [
      ...CATEGORY_ORDER.filter((c) => (counts.get(c) ?? 0) > 0),
      ...[...counts.keys()].filter((c) => !(CATEGORY_ORDER as readonly string[]).includes(c)),
    ];
    return ordered.map((c) => ({
      id: c,
      label: (
        <>
          {categoryLabel(c)} <span className="muted">({counts.get(c) ?? 0})</span>
        </>
      ),
    }));
  }, [allRows]);

  const filteredRules = useMemo(() => {
    const catRows = rowsForCategory(allRows, activeCategory);
    const q = filter.trim().toLowerCase();
    return catRows.filter((r) => {
      if (!ruleAtProtectLevel(r, activeProtect)) return false;
      if (!q) return true;
      return r.id.toLowerCase().includes(q) || r.name.toLowerCase().includes(q);
    });
  }, [allRows, activeCategory, activeProtect, filter]);

  const protectCounts = useMemo(() => {
    const catRows = rowsForCategory(allRows, activeCategory);
    return ([1, 2, 3] as ProtectLevel[]).map((p) => ({
      level: p,
      count: catRows.filter((r) => ruleAtProtectLevel(r, p)).length,
    }));
  }, [allRows, activeCategory]);

  const handleUpdate = async () => {
    const ok = await nerveConfirm({
      title: "更新最新规则集",
      message: "从官方规则仓库导入最新规则集？\n\n当前规则会先自动备份。下载与导入可能需要数分钟，请耐心等待。",
      okText: "更新",
    });
    if (!ok) return;
    setUpdating(true);
    try {
      const r = await apiPost<{
        rules: number;
        import: { rules_imported: number };
        backup_version: { id: string };
      }>("/api/rules/crs/update", { crs_version: "v4.25.0", paranoia_max: 2 }, 300_000);
      await nerveResult({
        type: "success",
        title: "更新成功",
        message: `已导入 ${r.import.rules_imported} 条规则，当前 ${r.rules} 条 L7 规则生效（备份版本 ${r.backup_version.id}）。`,
      });
      await refresh();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      await nerveResult({
        type: "error",
        title: "更新失败",
        message,
      });
    } finally {
      setUpdating(false);
    }
  };

  const handleRollback = async (versionId: string) => {
    const ok = await nerveConfirm({
      title: "回滚规则集",
      message: `回滚到版本 ${versionId}？\n\n回滚前会自动保存当前规则集。`,
      okText: "回滚",
      danger: true,
    });
    if (!ok) return;
    try {
      const r = await apiPost<{ rules: number }>("/api/rules/crs/rollback", { version_id: versionId });
      toast(`已回滚，当前 ${r.rules} 条 L7 规则生效`);
      await refresh();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      await nerveResult({
        type: "error",
        title: "回滚失败",
        message,
      });
    }
  };

  const pagePacks = paginate(packs, packsPage.page, packsPage.pageSize);
  const pageVersions = paginate(versions, versionsPage.page, versionsPage.pageSize);
  const pageRules = paginate(filteredRules, rulesPage.page, rulesPage.pageSize);

  return (
    <>
      <NervePageHeader
        title="核心规则库"
        actions={
          <NerveSyncActions>
            <NerveButton variant="primary" disabled={updating} onClick={() => void handleUpdate()}>
              {updating ? "更新中…" : "更新最新规则集"}
            </NerveButton>
            <NerveButton onClick={() => void refresh()}>刷新</NerveButton>
          </NerveSyncActions>
        }
      />

      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
      {loading && allRows.length === 0 ? <div className="page-loading">加载中…</div> : null}

      {!loading || allRows.length > 0 ? (
        <>
          <div
            className="dash-grid"
            style={{ gridTemplateColumns: "repeat(4,minmax(0,1fr))", marginBottom: 16 }}
          >
            <div className="stat-card">
              <div className="stat-card-value">{stats.activeN}</div>
              <div className="stat-card-label">当前生效规则</div>
            </div>
            <div className="stat-card">
              <div className="stat-card-value">{stats.highN}</div>
              <div className="stat-card-label">高危规则</div>
            </div>
            <div className="stat-card">
              <div className="stat-card-value">{stats.medN}</div>
              <div className="stat-card-label">中危规则</div>
            </div>
            <div className="stat-card">
              <div className="stat-card-value">{stats.lowN}</div>
              <div className="stat-card-label">低危规则</div>
            </div>
          </div>

          <NervePanelCard title="当前规则包">
            <p className="muted small">
              CRS 规则包 {packs.length} 个 · 共 {packs.reduce((n, p) => n + p.rule_count, 0)} 条
            </p>
            <NerveTable<PackSummary>
              rowKey="pack_id"
              dataSource={pagePacks}
              columns={[
                { key: "pack_id", title: "包 ID", render: (p) => <code>{p.pack_id}</code> },
                { key: "version", title: "版本", dataIndex: "version" },
                {
                  key: "enabled",
                  title: "状态",
                  render: (p) => (
                    <NerveTag tone={p.enabled ? "green" : "default"}>
                      {p.enabled ? "启用" : "禁用"}
                    </NerveTag>
                  ),
                },
                { key: "rule_count", title: "规则数", dataIndex: "rule_count" },
                {
                  key: "desc",
                  title: "说明",
                  render: (p) => <span className="muted">{p.description || "—"}</span>,
                },
              ]}
            />
            <NervePagination total={packs.length} {...packsPage} onChange={setPacksPage} />
          </NervePanelCard>

          <NervePanelCard title="版本历史">
            {versions.length === 0 ? (
              <p className="muted">暂无历史版本 — 更新后会自动创建</p>
            ) : (
              <>
                <NerveTable<CrsVersion>
                  rowKey="id"
                  dataSource={pageVersions}
                  columns={[
                    { key: "id", title: "版本 ID", render: (v) => <code>{v.id}</code> },
                    {
                      key: "created_at",
                      title: "时间",
                      render: (v) => new Date(v.created_at).toLocaleString("zh-CN"),
                    },
                    { key: "pack_count", title: "包数", dataIndex: "pack_count" },
                    { key: "rule_count", title: "规则数", dataIndex: "rule_count" },
                    {
                      key: "note",
                      title: "说明",
                      render: (v) => <span className="muted">{v.note || v.crs_version || "—"}</span>,
                    },
                    {
                      key: "actions",
                      title: "",
                      className: NERVE_COL_ACTIONS,
                      render: (v) => (
                        <NerveTableActions
                          actions={[
                            {
                              key: "rollback",
                              label: "回滚",
                              onClick: () => void handleRollback(v.id),
                            },
                          ]}
                        />
                      ),
                    },
                  ]}
                />
                <NervePagination total={versions.length} {...versionsPage} onChange={setVersionsPage} />
              </>
            )}
          </NervePanelCard>

          <NerveCard title="规则分级一览">
            <div className="table-toolbar" style={{ flexWrap: "wrap", gap: 12, marginBottom: 8 }}>
              {categoryTabs.length > 0 ? (
                <NerveTabBar tabs={categoryTabs} activeId={activeCategory} onChange={setActiveCategory} />
              ) : (
                <span className="muted">暂无规则</span>
              )}
              <NerveSearchInput
                placeholder="搜索 ID / 名称"
                value={filter}
                onChange={(e) => {
                  setFilter(e.target.value);
                  setRulesPage((p) => ({ ...p, page: 1 }));
                }}
                style={{ marginLeft: "auto" }}
              />
            </div>
            <div className="protect-level-bar">
              <span className="protect-level-label">拦截等级</span>
              <div className="protect-level-tabs">
                {protectCounts.map(({ level, count }) => (
                  <button
                    key={level}
                    type="button"
                    className={`protect-level-tab${level === activeProtect ? " active" : ""}`}
                    onClick={() => {
                      setActiveProtect(level);
                      setRulesPage((p) => ({ ...p, page: 1 }));
                    }}
                  >
                    {PROTECT_LABEL[level]} <span className="muted">({count})</span>
                  </button>
                ))}
              </div>
            </div>
            <NerveTable<RuleRow>
              rowKey="id"
              dataSource={pageRules}
              emptyText="暂无匹配规则"
              columns={[
                { key: "id", title: "ID", render: (r) => <code>{r.id}</code> },
                { key: "name", title: "名称", dataIndex: "name" },
                {
                  key: "severity",
                  title: "严重度",
                  render: (r) => (
                    <NerveTag tone={r.risk === "high" ? "red" : r.risk === "medium" ? "orange" : "blue"}>
                      {r.severity}
                    </NerveTag>
                  ),
                },
                {
                  key: "active",
                  title: "状态",
                  render: (r) => (
                    <NerveTag tone={r.active ? "green" : "default"}>{r.active ? "生效" : "未生效"}</NerveTag>
                  ),
                },
              ]}
            />
            <NervePagination total={filteredRules.length} {...rulesPage} onChange={setRulesPage} />
          </NerveCard>
        </>
      ) : null}
    </>
  );
}
