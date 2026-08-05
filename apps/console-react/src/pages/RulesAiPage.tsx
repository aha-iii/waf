import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NervePageHeader,
  NervePagination,
  NervePanelCard,
  NerveSyncActions,
  NerveTable,
  NerveTag,
  defaultPaginationState,
  paginate,
} from "@nerve/ui";
import { apiGet, apiPost } from "../api";
import { useToast } from "../hooks/useToast";
import { categoryLabel } from "../lib/labels";
import { PAGE_META } from "../nav";

type PackSummary = {
  pack_id: string;
  version: string;
  description?: string;
  enabled: boolean;
  rule_count: number;
};

type RuleSummary = {
  id: string;
  name: string;
  category: string;
  severity: string;
  source: string;
};

const CATEGORY_HELP: Record<string, string> = {
  prompt_injection: "Prompt 注入 / 越狱 — 针对 LLM 应用 API",
  ai_artifact: "AI 助手配置泄露 — .cursor/.claude 等目录访问",
};

export function RulesAiPage() {
  const toast = useToast();
  const [packs, setPacks] = useState<PackSummary[]>([]);
  const [rules, setRules] = useState<RuleSummary[]>([]);
  const [packsPage, setPacksPage] = useState(defaultPaginationState());
  const [rulesPage, setRulesPage] = useState(defaultPaginationState());

  const refresh = useCallback(async () => {
    const inv = await apiGet<{
      ai_packs: PackSummary[];
      rules: RuleSummary[];
    }>("/api/rules/packs?limit=500");
    const aiRules = (inv.rules || []).filter(
      (r) =>
        r.source.includes("/ai/") ||
        r.category.startsWith("prompt") ||
        r.category.startsWith("ai"),
    );
    setPacks(inv.ai_packs ?? []);
    setRules(aiRules);
    setPacksPage((p) => ({ ...p, page: 1 }));
    setRulesPage((p) => ({ ...p, page: 1 }));
  }, []);

  useEffect(() => {
    void refresh().catch((e) => toast(String(e), false));
  }, [refresh, toast]);

  const handleReload = async () => {
    try {
      const r = await apiPost<{ rules: number }>("/api/rules/packs", {});
      toast(`已热加载 ${r.rules} 条 L7 规则`);
      await refresh();
    } catch (e) {
      toast(String(e), false);
    }
  };

  const pagePacks = paginate(packs, packsPage.page, packsPage.pageSize);
  const pageRules = paginate(rules, rulesPage.page, rulesPage.pageSize);

  return (
    <>
      <NervePageHeader
        title={PAGE_META["rules-ai"].title}
        subtitle={PAGE_META["rules-ai"].subtitle}
        actions={
          <NerveSyncActions>
            <NerveButton variant="primary" onClick={() => void handleReload()}>
              热加载规则
            </NerveButton>
            <NerveButton onClick={() => void refresh().catch((e) => toast(String(e), false))}>
              刷新
            </NerveButton>
          </NerveSyncActions>
        }
      />

      <NervePanelCard title="规则包">
        {packs.length === 0 ? (
          <p className="muted">
            暂无规则包 — 请将 JSON 放入 <code>rules/ai/</code>
          </p>
        ) : (
          <>
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
          </>
        )}
      </NervePanelCard>

      <NervePanelCard title="规则明细">
        {rules.length === 0 ? (
          <div className="ant-empty">暂无规则 — 点击「热加载规则」</div>
        ) : (
          <>
            <NerveTable<RuleSummary>
              rowKey="id"
              dataSource={pageRules}
              columns={[
                { key: "id", title: "ID", render: (r) => <code>{r.id}</code> },
                { key: "name", title: "名称", dataIndex: "name" },
                {
                  key: "category",
                  title: "分类",
                  render: (r) => (
                    <div>
                      {categoryLabel(r.category)}
                      <br />
                      <span className="muted small">{CATEGORY_HELP[r.category] ?? ""}</span>
                    </div>
                  ),
                },
                { key: "severity", title: "严重度", dataIndex: "severity" },
              ]}
            />
            <NervePagination total={rules.length} {...rulesPage} onChange={setRulesPage} />
          </>
        )}
      </NervePanelCard>
    </>
  );
}
