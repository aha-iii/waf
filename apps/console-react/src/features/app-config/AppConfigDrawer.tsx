import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  NerveButton,
  NerveEmpty,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NerveSelect,
  NerveSwitch,
  NerveTabBar,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiDelete, apiGet, apiPost, apiPut } from "../../api";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "../../crs-preset";
import { useToast } from "../../hooks/useToast";
import {
  defaultPipelineEnabled,
  normalizePipelineEnabled,
  normalizePipelineOrder,
  PIPELINE_CAPABILITY_INTRO,
  type PipelineEnabledMap,
} from "../../lib/pipeline";
import {
  compactPipelineGraph,
  mergePipelineEditor,
  rebuildPipelineGraph,
} from "../../lib/pipeline-flow";
import { fetchAppCrsStatus, type AppCrsStatus } from "../../lib/app-crs";
import {
  mergeSemanticConfig,
  semanticConfigSnapshot,
  type SemanticConfig,
  type SemanticStats,
} from "../../lib/semantic";
import { SemanticConsole } from "../semantic/SemanticConsole";
import { AppCrsVersionBar } from "./AppCrsVersionBar";
import {
  isAppPrivateStrategy,
  isStrategyEnabled,
  strategyScopeLabel,
  type StrategyTemplate,
} from "../../lib/strategy";
import type { ModuleConfig, ProtectionModules } from "../../lib/waf-types";
import {
  buildCustomModuleEntries,
  CUSTOM_MODULE_KEY_PREFIX,
  MODULE_META,
  orderedPreAccessModuleEntries,
  orderedResponseModuleEntries,
  orderedWebAttackModuleEntries,
  type CustomRuleSetInfo,
} from "../../module-meta";
import { siteQuery, type ProtectedSite } from "../../sites";
import { CustomSetRulesModal } from "./CustomSetRulesModal";
import { CoreModuleSections } from "../modules/CoreModuleCard";
import { basicModuleDetailSupported } from "../modules/basic-module-detail";
import { BasicModuleDetailModal } from "../modules/BasicModuleDetailModal";
import { copyBasicModuleFields } from "../modules/basic-module-params";
import { moduleScopeHint } from "../modules/module-config-scope";
import { ModuleRulesModal } from "../modules/ModuleRulesModal";
import { AiProtectionPanel } from "./AiProtectionPanel";
import { PipelineList } from "./PipelineList";
import { PipelineFlowEditor, type PipelineFlowEditorHandle } from "./PipelineFlowEditor";

export type AppConfigDrawerProps = {
  open: boolean;
  site: ProtectedSite;
  sites: ProtectedSite[];
  siteIndex: number;
  onClose: () => void;
  onSitesChange: (sites: ProtectedSite[]) => void;
  onSaved: () => void;
};

type AppConfigTab = "basic" | "strategy" | "pipeline";
type ModSubTab = "semantic" | "pre_access" | "expert" | "core" | "custom" | "llm_app";

type ProtectionLevels = {
  scope?: string;
  semantic_mode?: string;
  expert_level: string;
  expert_level_inherited?: boolean;
  bound_strategy_id?: string;
};

const ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
  { value: "challenge", label: "挑战" },
];

const SENS_OPTS = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
];

const EXPERT_OPTS = [
  { value: "low", label: "低 · 高置信优先" },
  { value: "medium", label: "中 · 推荐默认" },
  { value: "high", label: "高 · 更全面覆盖" },
];

const SYNC_ICON = (
  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
    <path
      fill="currentColor"
      d="M13.65 2.35A7.96 7.96 0 0 0 8 0C3.58 0 0 3.58 0 8h1.5A6.5 6.5 0 0 1 12.6 3.4L10 6h6V0l-2.35 2.35zM2.35 13.65A7.96 7.96 0 0 0 8 16c4.42 0 8-3.58 8-8h-1.5A6.5 6.5 0 0 1 3.4 12.6L6 10H0v6l2.35-2.35z"
    />
  </svg>
);

function validateSiteId(site: ProtectedSite, allSites: ProtectedSite[], currentIndex: number): string | null {
  const id = site.id.trim();
  if (!id) return "应用 ID 不能为空";
  if (/\s/.test(id)) return "应用 ID 不能含空白字符";
  const dup = allSites.findIndex((s, i) => i !== currentIndex && s.id.trim() === id);
  if (dup >= 0) return `应用 ID「${id}」已存在，请更换`;
  return null;
}

function semanticSnapshot(cfg: SemanticConfig): string {
  const order = normalizePipelineOrder(cfg.engines.pipeline_order ?? []);
  const enabled = normalizePipelineEnabled(cfg.engines.pipeline_enabled);
  const editor = mergePipelineEditor(cfg.pipeline_editor);
  return JSON.stringify({
    core: semanticConfigSnapshot(cfg),
    pipeline_order: order,
    pipeline_enabled: enabled,
    pipeline_editor: {
      mode: editor.mode,
      layout: editor.layout,
    },
  });
}

function collectPipelineSemantic(
  semantic: SemanticConfig,
  editorMode: "list" | "flow",
  flowRef: PipelineFlowEditorHandle | null,
): SemanticConfig {
  const listOrder = normalizePipelineOrder(semantic.engines.pipeline_order ?? []);
  const listEnabled = normalizePipelineEnabled(semantic.engines.pipeline_enabled);
  const flowState =
    editorMode === "flow" && flowRef
      ? flowRef.readState()
      : { order: [], layout: {}, enabled: defaultPipelineEnabled(), graph: null };

  const pipelineOrder =
    editorMode === "list"
      ? listOrder
      : flowState.order.length
        ? normalizePipelineOrder(flowState.order)
        : listOrder;

  const pipelineEnabled =
    editorMode === "list"
      ? listEnabled
      : normalizePipelineEnabled(flowState.enabled);

  const rawGraph =
    editorMode === "list"
      ? rebuildPipelineGraph(pipelineOrder, pipelineEnabled)
      : (flowState.graph ??
        semantic.pipeline_editor?.graph ??
        rebuildPipelineGraph(pipelineOrder, pipelineEnabled));

  const graph =
    editorMode === "list" ? rawGraph : (compactPipelineGraph(rawGraph) ?? rawGraph);

  return {
    ...semantic,
    engines: {
      ...semantic.engines,
      pipeline_order: pipelineOrder,
      pipeline_enabled: pipelineEnabled,
    },
    pipeline_editor: {
      mode: editorMode,
      layout: { ...mergePipelineEditor(semantic.pipeline_editor).layout, ...flowState.layout },
      graph,
    },
  };
}

function applyStrategyTemplate(
  tpl: StrategyTemplate,
  prevModules: ProtectionModules,
  prevSemantic: SemanticConfig,
): { modules: ProtectionModules; semantic: SemanticConfig; levels: ProtectionLevels } {
  const tplModules = tpl.core?.modules ?? {};
  const modules: ProtectionModules = {
    version: tpl.core?.version ?? prevModules.version ?? 1,
    mode: tpl.core?.mode === "detect" ? "detect" : "block",
    modules: Object.fromEntries(
      Object.entries(tplModules).map(([k, v]) => [
        k,
        {
          enabled: !!v.enabled,
          action: v.action || "block",
          sensitivity: v.sensitivity || "medium",
          ...(v.disabled_rules?.length ? { disabled_rules: [...v.disabled_rules] } : {}),
          ...copyBasicModuleFields(v),
        },
      ]),
    ),
  };
  const mode = (tpl.semantic_mode || "standard") as SemanticConfig["mode"];
  const expert = (tpl.expert_level || "medium") as SemanticConfig["expert_level"];
  const semantic = { ...prevSemantic, mode, expert_level: expert };
  return {
    modules,
    semantic,
    levels: {
      scope: "app",
      semantic_mode: mode,
      expert_level: expert ?? "medium",
      expert_level_inherited: false,
      bound_strategy_id: tpl.id,
    },
  };
}

async function fetchStrategyTemplates(siteId: string): Promise<StrategyTemplate[]> {
  try {
    const r = await apiGet<{ templates: StrategyTemplate[] }>(
      `/api/protection-strategies${siteQuery(siteId)}`,
    );
    return r.templates ?? [];
  } catch {
    return [];
  }
}

type ModuleCardProps = {
  entryKey: string;
  title: string;
  desc: string;
  cfg: ModuleConfig;
  readonly: boolean;
  crsCat?: string | null;
  customSetId?: string;
  onChange: (patch: Partial<ModuleConfig>) => void;
  onOpenRules: () => void;
};

function ModuleCard({
  entryKey,
  title,
  desc,
  cfg,
  readonly,
  crsCat,
  customSetId,
  onChange,
  onOpenRules,
}: ModuleCardProps) {
  const showRules = Boolean(crsCat || customSetId);
  return (
    <div
      className={`protection-module-card app-mod-card${cfg.enabled ? "" : " mod-card-disabled"}${readonly ? " is-readonly" : ""}`}
      data-key={entryKey}
    >
      <div className="protection-module-head">
        <strong>{title}</strong>
        <div className="protection-module-head-actions">
          {showRules ? (
            <NerveButton size="sm" variant="link" onClick={onOpenRules}>
              {readonly ? "查看规则" : "规则集"}
            </NerveButton>
          ) : null}
          <NerveSwitch
            checked={cfg.enabled}
            disabled={readonly}
            onChange={(e) => onChange({ enabled: e.target.checked })}
          />
        </div>
      </div>
      <p className="muted small">
        {desc}
        {cfg.disabled_rules?.length
          ? ` · 已禁用 ${cfg.disabled_rules.length} 条`
          : ""}
      </p>
      <div className="protection-module-actions">
        <NerveSelect
          value={cfg.action}
          options={ACTION_OPTS}
          disabled={readonly}
          onChange={(e) => onChange({ action: e.target.value })}
        />
        <NerveSelect
          value={cfg.sensitivity}
          options={SENS_OPTS}
          disabled={readonly}
          onChange={(e) => onChange({ sensitivity: e.target.value })}
        />
      </div>
    </div>
  );
}

export function AppConfigDrawer({
  open,
  site: siteProp,
  sites: sitesProp,
  siteIndex: siteIndexProp,
  onClose,
  onSitesChange,
  onSaved,
}: AppConfigDrawerProps) {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<AppConfigTab>("basic");
  const [modSubTab, setModSubTab] = useState<ModSubTab>("core");
  const [saving, setSaving] = useState(false);

  const [site, setSite] = useState<ProtectedSite>(siteProp);
  const [sites, setSites] = useState(sitesProp);
  const [siteIndex, setSiteIndex] = useState(siteIndexProp);

  const [modules, setModules] = useState<ProtectionModules>({ version: 1, mode: "block", modules: {} });
  const [semantic, setSemantic] = useState<SemanticConfig>(() => mergeSemanticConfig(undefined));
  const [customSets, setCustomSets] = useState<CustomRuleSetInfo[]>([]);
  const [protectionLevels, setProtectionLevels] = useState<ProtectionLevels | null>(null);
  const [strategyTemplates, setStrategyTemplates] = useState<StrategyTemplate[]>([]);
  const [selectedStrategyId, setSelectedStrategyId] = useState<string | null>("tpl-standard");

  const [initialSiteJson, setInitialSiteJson] = useState("");
  const [initialModulesJson, setInitialModulesJson] = useState("");
  const [initialSemanticJson, setInitialSemanticJson] = useState("");
  const [initialBoundStrategyId, setInitialBoundStrategyId] = useState("tpl-standard");
  /** 本应用运行时快照：选中内置策略时回显，避免被本应用私有策略预览污染 */
  const [appRuntimeModules, setAppRuntimeModules] = useState<ProtectionModules | null>(null);
  const [appRuntimeSemantic, setAppRuntimeSemantic] = useState<SemanticConfig | null>(null);
  const [appRuntimeLevels, setAppRuntimeLevels] = useState<ProtectionLevels | null>(null);

  const [promptOpen, setPromptOpen] = useState(false);
  const [promptMode, setPromptMode] = useState<"create" | "edit">("create");
  const [promptTargetId, setPromptTargetId] = useState<string | null>(null);
  const [promptName, setPromptName] = useState("");
  const [promptDesc, setPromptDesc] = useState("");

  const [rulesModal, setRulesModal] = useState<{
    category: string;
    protect: ReturnType<typeof sensitivityToProtectLevel>;
    title: string;
    key: string;
    disabled: string[];
  } | null>(null);

  const [basicDetailKey, setBasicDetailKey] = useState<string | null>(null);

  const [customRulesModal, setCustomRulesModal] = useState<{
    setId: string;
    setName: string;
  } | null>(null);

  const [semanticStats, setSemanticStats] = useState<SemanticStats | null>(null);
  const [crsStatus, setCrsStatus] = useState<AppCrsStatus | null>(null);

  const flowEditorRef = useRef<PipelineFlowEditorHandle>(null);

  const selectedStrategy = useMemo(
    () => strategyTemplates.find((t) => t.id === selectedStrategyId),
    [strategyTemplates, selectedStrategyId],
  );
  const modulesReadonly = !isAppPrivateStrategy(selectedStrategy);
  const inSemanticSettings = modSubTab === "semantic";

  const refreshSemanticStats = useCallback(async () => {
    try {
      setSemanticStats(await apiGet<SemanticStats>(`/api/semantic/stats${siteQuery(site.id)}`));
      toast("观测数据已刷新");
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
    }
  }, [site.id, toast]);

  const openSemanticSettings = useCallback(async () => {
    setModSubTab("semantic");
    if (!semanticStats) {
      try {
        setSemanticStats(
          await apiGet<SemanticStats>(`/api/semantic/stats${siteQuery(site.id)}`).catch(() => null),
        );
      } catch {
        /* ignore */
      }
    }
  }, [semanticStats, site.id]);

  const handleCrsApplied = useCallback(async () => {
    try {
      setCrsStatus(await fetchAppCrsStatus(site.id));
      setModules(await apiGet<ProtectionModules>(`/api/modules${siteQuery(site.id)}`));
    } catch {
      /* keep current */
    }
  }, [site.id]);

  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const sid = siteProp.id;
      const [
        modulesRes,
        semanticRes,
        customRulesRes,
        levelsRes,
        tplRes,
        crsRes,
      ] = await Promise.all([
        apiGet<ProtectionModules>(`/api/modules${siteQuery(sid)}`),
        apiGet<{ config: Partial<SemanticConfig> }>(`/api/semantic/config${siteQuery(sid)}`).catch(
          () => ({ config: undefined }),
        ),
        apiGet<{ sets: CustomRuleSetInfo[] }>(`/api/custom-rules${siteQuery(sid)}`).catch(() => ({
          sets: [],
        })),
        apiGet<ProtectionLevels>(`/api/protection-strategy${siteQuery(sid)}`).catch(() =>
          apiGet<ProtectionLevels>(`/api/protection-levels${siteQuery(sid)}`).catch(() => null),
        ),
        fetchStrategyTemplates(sid),
        fetchAppCrsStatus(sid).catch(() => null),
      ]);

      const mergedSem = mergeSemanticConfig(semanticRes.config);
      mergedSem.engines.pipeline_order = normalizePipelineOrder(
        mergedSem.engines.pipeline_order ?? [],
      );
      mergedSem.engines.pipeline_enabled = normalizePipelineEnabled(
        mergedSem.engines.pipeline_enabled ?? defaultPipelineEnabled(),
      );

      let bound = levelsRes?.bound_strategy_id ?? "tpl-standard";
      const boundTpl = tplRes.find((t) => t.id === bound);
      if (!boundTpl || !isStrategyEnabled(boundTpl)) bound = "tpl-standard";

      setSite({ ...siteProp });
      setSites([...sitesProp]);
      setSiteIndex(siteIndexProp);
      setModules(modulesRes);
      setSemantic(mergedSem);
      setCustomSets(customRulesRes.sets ?? []);
      setProtectionLevels(levelsRes);
      setStrategyTemplates(tplRes);
      setSelectedStrategyId(bound);
      setInitialBoundStrategyId(bound);
      setInitialSiteJson(JSON.stringify(siteProp));
      setInitialModulesJson(JSON.stringify(modulesRes));
      setInitialSemanticJson(semanticSnapshot(mergedSem));
      setAppRuntimeModules(modulesRes);
      setAppRuntimeSemantic(mergedSem);
      setAppRuntimeLevels(levelsRes);
      setSemanticStats(null);
      setCrsStatus(crsRes);
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
      onClose();
    } finally {
      setLoading(false);
    }
  }, [siteProp, sitesProp, siteIndexProp, toast, onClose]);

  const drawerWasOpenRef = useRef(false);
  useEffect(() => {
    if (open && !drawerWasOpenRef.current) {
      setTab("basic");
      setModSubTab("core");
    }
    drawerWasOpenRef.current = open;
  }, [open]);

  useEffect(() => {
    if (!open) return;
    void loadAll();
  }, [open, loadAll]);

  const upsertStrategyCache = (tpl: StrategyTemplate) => {
    setStrategyTemplates((prev) => {
      const idx = prev.findIndex((x) => x.id === tpl.id);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = tpl;
        return next;
      }
      return [...prev, tpl];
    });
  };

  const updateModule = (key: string, patch: Partial<ModuleConfig>) => {
    if (modulesReadonly) return;
    setModules((prev) => {
      const cur = prev.modules[key] ?? {
        enabled: false,
        action: "block",
        sensitivity: "medium",
      };
      return {
        ...prev,
        modules: {
          ...prev.modules,
          [key]: { ...cur, ...patch },
        },
      };
    });
  };

  const selectStrategy = async (id: string) => {
    const cached = strategyTemplates.find((t) => t.id === id);
    if (cached && !isStrategyEnabled(cached)) {
      toast("该策略已禁用，请先启用或选择其他策略", false);
      return;
    }
    setSelectedStrategyId(id);

    if (isAppPrivateStrategy(cached)) {
      let tpl = cached!;
      try {
        if (!cached) {
          tpl = await apiGet<StrategyTemplate>(`/api/protection-strategies/${encodeURIComponent(id)}`);
          if (!isStrategyEnabled(tpl)) {
            upsertStrategyCache(tpl);
            toast("该策略已禁用，请先启用或选择其他策略", false);
            return;
          }
          upsertStrategyCache(tpl);
        }
        if (!isAppPrivateStrategy(tpl)) {
          // 非本应用私有：回显运行时
          if (appRuntimeModules) setModules(appRuntimeModules);
          if (appRuntimeSemantic) setSemantic(appRuntimeSemantic);
          if (appRuntimeLevels) setProtectionLevels(appRuntimeLevels);
          return;
        }
        const applied = applyStrategyTemplate(tpl, modules, semantic);
        setModules(applied.modules);
        setSemantic(applied.semantic);
        setProtectionLevels(applied.levels);
      } catch {
        toast("加载策略快照失败", false);
      }
      return;
    }

    // 内置/全局：回显本应用运行时（隔离，不同步写入）
    if (appRuntimeModules) setModules(appRuntimeModules);
    if (appRuntimeSemantic) setSemantic(appRuntimeSemantic);
    if (appRuntimeLevels) setProtectionLevels(appRuntimeLevels);
  };

  const syncBuiltinStrategy = async (id: string) => {
    const tpl =
      strategyTemplates.find((t) => t.id === id) ??
      (await apiGet<StrategyTemplate>(`/api/protection-strategies/${encodeURIComponent(id)}`).catch(
        () => null,
      ));
    if (!tpl?.builtin) {
      await nerveResult({ type: "info", title: "无法同步", message: "仅内置策略支持从策略库同步。" });
      return;
    }
    if (!isStrategyEnabled(tpl)) {
      await nerveResult({ type: "info", title: "无法同步", message: "该内置策略已禁用。" });
      return;
    }
    const name = tpl.name || id;
    const ok = await nerveConfirm({
      title: "同步内置策略",
      message: `将用策略库最新配置覆盖本应用「${site.name}」当前防护设置？此操作会立即写入并生效。`,
      okText: "同步",
    });
    if (!ok) return;
    try {
      const latest = await apiGet<StrategyTemplate>(
        `/api/protection-strategies/${encodeURIComponent(id)}`,
      );
      if (!latest?.builtin) throw new Error("策略不是内置模板，无法同步");
      if (!isStrategyEnabled(latest)) throw new Error("该内置策略已禁用");
      await apiPost(`/api/protection-strategies/${encodeURIComponent(latest.id)}/apply`, {
        target: "app",
        site_id: site.id,
      });
      const [modulesRes, semanticRes, levelsRes] = await Promise.all([
        apiGet<ProtectionModules>(`/api/modules${siteQuery(site.id)}`),
        apiGet<{ config: Partial<SemanticConfig> }>(`/api/semantic/config${siteQuery(site.id)}`),
        apiGet<ProtectionLevels>(`/api/protection-strategy${siteQuery(site.id)}`).catch(() => null),
      ]);
      const merged = mergeSemanticConfig(semanticRes.config);
      merged.engines.pipeline_order = normalizePipelineOrder(merged.engines.pipeline_order ?? []);
      merged.engines.pipeline_enabled = normalizePipelineEnabled(
        merged.engines.pipeline_enabled ?? defaultPipelineEnabled(),
      );
      setModules(modulesRes);
      setSemantic(merged);
      if (levelsRes) setProtectionLevels(levelsRes);
      setAppRuntimeModules(modulesRes);
      setAppRuntimeSemantic(merged);
      setAppRuntimeLevels(levelsRes);
      upsertStrategyCache(latest);
      setSelectedStrategyId(latest.id);
      setInitialModulesJson(JSON.stringify(modulesRes));
      setInitialSemanticJson(semanticSnapshot(merged));
      setInitialBoundStrategyId(latest.id);
      if (site.id === "default") void apiPost("/api/sync").catch(() => null);
      toast(`已从策略库同步「${name}」到本应用`);
      onSaved();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
    }
  };

  const openCreateStrategy = () => {
    setPromptMode("create");
    setPromptTargetId(null);
    setPromptName("");
    setPromptDesc("");
    setPromptOpen(true);
  };

  const openEditStrategy = (id: string) => {
    const tpl = strategyTemplates.find((t) => t.id === id);
    setPromptMode("edit");
    setPromptTargetId(id);
    setPromptName(tpl?.name ?? "");
    setPromptDesc(tpl?.description ?? "");
    setPromptOpen(true);
  };

  const submitPrompt = async () => {
    const name = promptName.trim();
    if (!name) {
      toast("请填写名称", false);
      return;
    }
    if (promptMode === "create") {
      const ok = await nerveConfirm({
        title: "新建本应用策略",
        message: `创建「${name}」？`,
        okText: "创建",
      });
      if (!ok) return;
      try {
        const resp = await apiPost<{ ok: boolean; template: StrategyTemplate }>(
          "/api/protection-strategies",
          {
            name,
            description: promptDesc.trim() || "",
            site_id: site.id,
            source_id: selectedStrategyId || undefined,
          },
        );
        if (!resp.template) throw new Error("未返回策略");
        if (!isAppPrivateStrategy(resp.template)) {
          throw new Error("创建失败：未写入本应用策略，请重启 API 后重试");
        }
        upsertStrategyCache(resp.template);
        const applied = applyStrategyTemplate(resp.template, modules, semantic);
        setModules(applied.modules);
        setSemantic(applied.semantic);
        setProtectionLevels(applied.levels);
        setSelectedStrategyId(resp.template.id);
        setPromptOpen(false);
        toast(`「${name}」已创建`);
      } catch (e) {
        toast(String(e), false);
      }
      return;
    }
    if (!promptTargetId) return;
    const ok = await nerveConfirm({
      title: "编辑本应用策略",
      message: `保存「${name}」的名称与说明？`,
      okText: "保存",
    });
    if (!ok) return;
    try {
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(promptTargetId)}`,
        { name, description: promptDesc.trim() || "" },
      );
      if (!resp.template) throw new Error("未返回策略");
      upsertStrategyCache(resp.template);
      setPromptOpen(false);
      toast("已保存");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const toggleStrategyEnabled = async (id: string, next: boolean) => {
    const tpl = strategyTemplates.find((t) => t.id === id);
    const ok = await nerveConfirm({
      title: next ? "启用策略" : "禁用策略",
      message: next
        ? `启用「${tpl?.name ?? id}」？`
        : `禁用「${tpl?.name ?? id}」？禁用后不可再选绑；若当前绑定该策略将回退到「标准」。`,
      okText: next ? "启用" : "禁用",
      danger: !next,
    });
    if (!ok) return;
    try {
      const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
        `/api/protection-strategies/${encodeURIComponent(id)}`,
        { enabled: next },
      );
      if (!resp.template) throw new Error("未返回策略");
      upsertStrategyCache(resp.template);
      if (!next && selectedStrategyId === id) {
        if (initialBoundStrategyId === id) setInitialBoundStrategyId("tpl-standard");
        setSelectedStrategyId("tpl-standard");
      }
      toast(next ? "已启用" : "已禁用");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const deleteStrategy = async (id: string) => {
    const tpl = strategyTemplates.find((t) => t.id === id);
    const ok = await nerveConfirm({
      title: "删除策略",
      message: `确定删除「${tpl?.name ?? id}」？删除后不可恢复；若当前绑定该策略将回退到「标准」。`,
      okText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/protection-strategies/${encodeURIComponent(id)}`);
      setStrategyTemplates((prev) => prev.filter((t) => t.id !== id));
      if (selectedStrategyId === id) {
        if (initialBoundStrategyId === id) setInitialBoundStrategyId("tpl-standard");
        setSelectedStrategyId("tpl-standard");
      }
      toast("已删除");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const saveAll = async () => {
    if (!site.name.trim()) {
      toast("请填写名称", false);
      setTab("basic");
      return;
    }
    if (!site.hosts.length) {
      toast("请填写 Host", false);
      setTab("basic");
      return;
    }
    const idErr = validateSiteId(site, sites, siteIndex);
    if (idErr) {
      toast(idErr, false);
      setTab("basic");
      return;
    }
    const siteDirty = JSON.stringify(site) !== initialSiteJson;
    let modulesDirty = JSON.stringify(modules) !== initialModulesJson;
    const semanticToSave = collectPipelineSemantic(
      semantic,
      pipelineEditorMode,
      flowEditorRef.current,
    );
    let semanticDirty = semanticSnapshot(semanticToSave) !== initialSemanticJson;
    const bindingDirty = (selectedStrategyId ?? "") !== initialBoundStrategyId;
    const privateTpl = !modulesReadonly && !!selectedStrategyId;
    if (privateTpl && (modulesDirty || semanticDirty || bindingDirty)) {
      modulesDirty = true;
    }
    if (!siteDirty && !modulesDirty && !semanticDirty && !bindingDirty) {
      await nerveResult({ type: "info", title: "无需保存", message: "没有需要保存的变更" });
      return;
    }

    const ok = await nerveConfirm({
      title: "保存并生效",
      message: `确定保存应用「${site.name}」配置并生效吗？`,
      okText: "保存",
    });
    if (!ok) return;

    setSaving(true);
    const errors: string[] = [];
    let nextSites = [...sites];
    try {
      // 任意配置保存时都写入完整应用列表，避免新应用只改模块却未持久化
      if (siteIndex >= 0) nextSites[siteIndex] = site;
      else {
        nextSites.push(site);
        setSiteIndex(nextSites.length - 1);
      }
      setSites(nextSites);
      onSitesChange(nextSites);
      try {
        await apiPut("/api/sites", nextSites);
        setInitialSiteJson(JSON.stringify(site));
      } catch (e) {
        errors.push(`基础信息：${e instanceof Error ? e.message : String(e)}`);
      }

      if (privateTpl && selectedStrategyId && (modulesDirty || semanticDirty)) {
        try {
          const resp = await apiPut<{ ok: boolean; template: StrategyTemplate }>(
            `/api/protection-strategies/${encodeURIComponent(selectedStrategyId)}`,
            {
              core: modules,
              semantic_mode: semantic.mode,
              expert_level: semantic.expert_level ?? "medium",
              expert: { level: semantic.expert_level ?? "medium" },
            },
          );
          if (resp.template) upsertStrategyCache(resp.template);
        } catch (e) {
          errors.push(`本应用策略：${e instanceof Error ? e.message : String(e)}`);
        }
      }

      if (modulesDirty) {
        try {
          await apiPut(`/api/modules${siteQuery(site.id)}`, modules);
          setInitialModulesJson(JSON.stringify(modules));
          setAppRuntimeModules(modules);
        } catch (e) {
          errors.push(`防护模块：${e instanceof Error ? e.message : String(e)}`);
        }
      }

      if (semanticDirty) {
        try {
          await apiPut(`/api/semantic/config${siteQuery(site.id)}`, semanticToSave, 20_000);
          setSemantic(semanticToSave);
          setProtectionLevels({
            scope: "app",
            semantic_mode: semanticToSave.mode,
            expert_level: semanticToSave.expert_level ?? "medium",
            expert_level_inherited: false,
            bound_strategy_id: selectedStrategyId ?? undefined,
          });
          setInitialSemanticJson(semanticSnapshot(semanticToSave));
          setAppRuntimeSemantic(semanticToSave);
          setAppRuntimeLevels({
            scope: "app",
            semantic_mode: semanticToSave.mode,
            expert_level: semanticToSave.expert_level ?? "medium",
            expert_level_inherited: false,
            bound_strategy_id: selectedStrategyId ?? undefined,
          });
        } catch (e) {
          errors.push(`检测流水线：${e instanceof Error ? e.message : String(e)}`);
        }
      }

      if ((bindingDirty || privateTpl || modulesDirty || semanticDirty) && selectedStrategyId) {
        try {
          await apiPut(`/api/protection-strategy${siteQuery(site.id)}`, {
            bound_strategy_id: selectedStrategyId,
          });
          setInitialBoundStrategyId(selectedStrategyId);
          setProtectionLevels((prev) => ({
            ...(prev ?? {
              scope: "app",
              semantic_mode: semantic.mode,
              expert_level: semantic.expert_level ?? "medium",
            }),
            bound_strategy_id: selectedStrategyId,
          }));
        } catch (e) {
          errors.push(`策略绑定：${e instanceof Error ? e.message : String(e)}`);
        }
      }

      if (errors.length > 0) throw new Error(errors.join("；"));

      if (site.id === "default" && (modulesDirty || semanticDirty)) {
        void apiPost("/api/sync").catch(() => null);
      }
      toast(`应用「${site.name}」已保存`);
      onSaved();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
    } finally {
      setSaving(false);
    }
  };

  const attackEntries = orderedWebAttackModuleEntries(modules.modules);
  const preAccessEntries = orderedPreAccessModuleEntries(modules.modules);
  const responseEntries = orderedResponseModuleEntries(modules.modules);
  const customEntries = buildCustomModuleEntries(modules.modules, customSets, {
    onlyMounted: modulesReadonly,
  });

  const pipelineOrder = normalizePipelineOrder(semantic.engines.pipeline_order ?? []);
  const pipelineEnabled = normalizePipelineEnabled(
    (semantic.engines.pipeline_enabled as PipelineEnabledMap) ?? defaultPipelineEnabled(),
  );
  const pipelineEditor = mergePipelineEditor(semantic.pipeline_editor);
  const pipelineEditorMode = pipelineEditor.mode;

  const switchPipelineMode = (newMode: "list" | "flow") => {
    if (pipelineEditorMode === newMode) return;
    setSemantic((s) => {
      const collected = collectPipelineSemantic(s, pipelineEditorMode, flowEditorRef.current);
      if (newMode === "flow") {
        const order = normalizePipelineOrder(collected.engines.pipeline_order ?? []);
        const enabled = normalizePipelineEnabled(collected.engines.pipeline_enabled);
        return {
          ...collected,
          pipeline_editor: {
            ...mergePipelineEditor(collected.pipeline_editor),
            mode: newMode,
            graph:
              collected.pipeline_editor?.graph ?? rebuildPipelineGraph(order, enabled),
          },
        };
      }
      return {
        ...collected,
        pipeline_editor: { ...mergePipelineEditor(collected.pipeline_editor), mode: newMode },
      };
    });
  };

  if (!open) return null;

  return createPortal(
    <>
      <div
        className="app-config-overlay"
        onClick={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <div className="app-config-drawer" onClick={(e) => e.stopPropagation()}>
          <div className="app-config-head">
            <div>
              <h3>应用配置</h3>
              <p className="muted small">
                {site.name.trim() || "未命名应用"} · <code>{site.id}</code>
              </p>
            </div>
            <NerveButton variant="link" onClick={onClose}>
              ✕
            </NerveButton>
          </div>

          <nav className="app-config-tabs">
            {(
              [
                ["basic", "① 基础信息"],
                ["strategy", "② 防护策略"],
                ["pipeline", "③ 检测流水线"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                className={`app-config-tab${tab === id ? " active" : ""}`}
                onClick={() => {
                  setTab(id);
                  if (id === "strategy") {
                    void apiGet<{ sets: CustomRuleSetInfo[] }>(
                      `/api/custom-rules${siteQuery(site.id)}`,
                    )
                      .then((d) => setCustomSets(d.sets ?? []))
                      .catch(() => null);
                  }
                }}
              >
                {label}
              </button>
            ))}
          </nav>

          <div className="app-config-body">
            {loading ? (
              <div className="muted" style={{ padding: 48, textAlign: "center" }}>
                加载配置…
              </div>
            ) : null}

            {!loading && tab === "basic" ? (
              <NerveForm className="form-stack" style={{ maxWidth: 560 }}>
                <NerveFormItem label="应用 ID" required>
                  <NerveInput
                    wide
                    value={site.id}
                    readOnly={site.id === "default" || siteProp.id === "default"}
                    onChange={(e) => setSite((s) => ({ ...s, id: e.target.value }))}
                  />
                </NerveFormItem>
                <NerveFormItem label="名称" required>
                  <NerveInput
                    wide
                    value={site.name}
                    placeholder="请输入名称"
                    onChange={(e) => setSite((s) => ({ ...s, name: e.target.value }))}
                  />
                </NerveFormItem>
                <NerveFormItem label="Host（逗号分隔）" required>
                  <NerveInput
                    wide
                    value={site.hosts.join(",")}
                    placeholder="请输入 Host，多个用逗号分隔"
                    onChange={(e) =>
                      setSite((s) => ({
                        ...s,
                        hosts: e.target.value
                          .split(",")
                          .map((h) => h.trim())
                          .filter(Boolean),
                      }))
                    }
                  />
                </NerveFormItem>
                <NerveFormItem label="备注">
                  <NerveInput
                    wide
                    value={site.description ?? site.upstream ?? ""}
                    placeholder="上游 URL 或说明"
                    onChange={(e) =>
                      setSite((s) => ({
                        ...s,
                        description: e.target.value || undefined,
                        upstream: e.target.value || undefined,
                      }))
                    }
                  />
                </NerveFormItem>
                <NerveFormItem label="启用防护">
                  <NerveSwitch
                    checked={site.enabled}
                    onChange={(e) => setSite((s) => ({ ...s, enabled: e.target.checked }))}
                  />
                </NerveFormItem>
              </NerveForm>
            ) : null}

            {!loading && tab === "strategy" ? (
              <>
                <div className="app-strategy-picker" role="radiogroup" aria-label="防护策略">
                  <div className="app-strategy-picker-head">
                    <div className="app-strategy-picker-label">选择防护策略</div>
                    <NerveButton size="sm" onClick={openCreateStrategy}>
                      新建本应用策略
                    </NerveButton>
                  </div>
                  {strategyTemplates.length === 0 ? (
                    <div className="app-strategy-picker-empty muted">暂无可用策略</div>
                  ) : (
                    <div className="app-strategy-picker-grid">
                      {strategyTemplates.map((t) => {
                        const active = t.id === selectedStrategyId;
                        const enabled = isStrategyEnabled(t);
                        const isApp = isAppPrivateStrategy(t);
                        const scope = strategyScopeLabel(t);
                        if (!isApp) {
                          return (
                            <div
                              key={t.id}
                              className={`app-strategy-chip app-strategy-chip--builtin${active ? " active" : ""}${enabled ? "" : " is-disabled"}`}
                            >
                              <button
                                type="button"
                                className="app-strategy-chip-main"
                                disabled={!enabled}
                                onClick={() => void selectStrategy(t.id)}
                              >
                                <strong>{t.name}</strong>
                                <span className="muted small">
                                  {scope}
                                  {enabled ? "" : " · 已禁用"}
                                </span>
                              </button>
                              <div className="app-strategy-chip-ops">
                                <button
                                  type="button"
                                  className="app-strategy-chip-sync"
                                  title="从策略库同步到本应用"
                                  aria-label="从策略库同步到本应用"
                                  disabled={!enabled}
                                  onClick={() => void syncBuiltinStrategy(t.id)}
                                >
                                  {SYNC_ICON}
                                </button>
                              </div>
                            </div>
                          );
                        }
                        return (
                          <div
                            key={t.id}
                            className={`app-strategy-chip app-strategy-chip--app${active ? " active" : ""}${enabled ? "" : " is-disabled"}`}
                          >
                            <button
                              type="button"
                              className="app-strategy-chip-main"
                              disabled={!enabled}
                              onClick={() => void selectStrategy(t.id)}
                            >
                              <strong>{t.name}</strong>
                              <span className="muted small">
                                {scope}
                                {enabled ? "" : " · 已禁用"}
                              </span>
                            </button>
                            <div className="app-strategy-chip-ops">
                              <NerveButton
                                size="sm"
                                variant="link"
                                onClick={() => openEditStrategy(t.id)}
                              >
                                编辑
                              </NerveButton>
                              <NerveButton
                                size="sm"
                                variant="link"
                                onClick={() => void toggleStrategyEnabled(t.id, !enabled)}
                              >
                                {enabled ? "禁用" : "启用"}
                              </NerveButton>
                              <NerveButton
                                size="sm"
                                variant="link"
                                className="app-strategy-chip-del"
                                onClick={() => void deleteStrategy(t.id)}
                              >
                                删除
                              </NerveButton>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>

                <div className={`app-strategy-modules${modulesReadonly ? " is-readonly" : ""}`}>
                  {modulesReadonly ? (
                    <p className="app-strategy-isolate-hint muted small">
                      下方为<strong>本应用当前生效</strong>的配置；策略库变更不会自动写入。修改内置策略后，点卡片上的同步图标写入本应用。
                    </p>
                  ) : null}

                  <div className="app-config-section-head">
                    <label>
                      防护模式{" "}
                      <NerveSelect
                        value={modules.mode}
                        disabled={modulesReadonly}
                        style={{ marginLeft: 8, width: 200 }}
                        options={[
                          { value: "block", label: "拦截模式" },
                          { value: "detect", label: "检测模式（仅记录）" },
                        ]}
                        onChange={(e) =>
                          setModules((m) => ({ ...m, mode: e.target.value }))
                        }
                      />
                    </label>
                  </div>

                  <nav className="mod-group-tabs" role="tablist" aria-label="防护分区">
                    <button
                      type="button"
                      className={`mod-group-tab${inSemanticSettings ? " active" : ""}`}
                      onClick={() => void openSemanticSettings()}
                    >
                      防护设置
                    </button>
                    <button
                      type="button"
                      className={`mod-group-tab${!inSemanticSettings ? " active" : ""}`}
                      onClick={() => {
                        if (modSubTab === "semantic") setModSubTab("core");
                      }}
                    >
                      防护模块
                    </button>
                  </nav>

                  {inSemanticSettings ? (
                    <div className="mod-sub-panels">
                      <div className="mod-sub-panel" data-mod-sub-panel="semantic">
                        <SemanticConsole
                          compact
                          cfg={semantic}
                          onChange={setSemantic}
                          stats={semanticStats}
                          siteId={site.id}
                          readonly={modulesReadonly}
                          onRefreshStats={modulesReadonly ? undefined : refreshSemanticStats}
                          onFeedbackConfigUpdated={(next) => setSemantic(mergeSemanticConfig(next))}
                        />
                      </div>
                    </div>
                  ) : (
                    <>
                      <NerveTabBar
                        tabs={[
                          { id: "pre_access", label: "基础防护" },
                          { id: "expert", label: "专家防护" },
                          { id: "core", label: "核心防护" },
                          { id: "custom", label: "自定义防护" },
                          { id: "llm_app", label: "AI防护" },
                        ]}
                        activeId={modSubTab}
                        onChange={(id) => {
                          setModSubTab(id as ModSubTab);
                          if (id === "custom") {
                            void apiGet<{ sets: CustomRuleSetInfo[] }>(
                              `/api/custom-rules${siteQuery(site.id)}`,
                            )
                              .then((d) => setCustomSets(d.sets ?? []))
                              .catch(() => null);
                          }
                        }}
                      />

                      {modSubTab === "pre_access" ? (
                        <>
                          <p className="muted small" style={{ marginBottom: 12 }}>
                            接入层引擎：在深度检测前完成 CC 限流、Bot 识别等前置过滤。
                            {moduleScopeHint("app") ? ` ${moduleScopeHint("app")}` : ""}
                          </p>
                          <CoreModuleSections
                            attack={[]}
                            preAccess={preAccessEntries}
                            response={responseEntries}
                            readonly={modulesReadonly}
                            configScope="app"
                            onChange={(key, patch) => updateModule(key, patch)}
                            onOpenDetail={(key) => {
                              if (basicModuleDetailSupported(key)) setBasicDetailKey(key);
                            }}
                          />
                        </>
                      ) : null}

                      {modSubTab === "expert" ? (
                        <div className="module-grid app-config-mod-grid">
                          <p className="muted small" style={{ gridColumn: "1 / -1", margin: "0 0 8px" }}>
                            专家引擎在规则匹配基础上叠加语义解码与自适应评分（用 AI 增强 Web 应用防护），按强度档平衡检出率与误报。
                          </p>
                          <div className="protection-module-card pl-expert-card">
                            <div className="protection-module-head">
                              <strong>专家防护强度</strong>
                            </div>
                            <div className="protection-module-actions">
                              <NerveSelect
                                value={
                                  protectionLevels?.expert_level ||
                                  semantic.expert_level ||
                                  "medium"
                                }
                                disabled={modulesReadonly}
                                options={EXPERT_OPTS}
                                onChange={(e) => {
                                  const v = e.target.value as SemanticConfig["expert_level"];
                                  setSemantic((s) => ({ ...s, expert_level: v }));
                                  setProtectionLevels((prev) => ({
                                    ...(prev ?? { expert_level: v ?? "medium" }),
                                    expert_level: v ?? "medium",
                                    expert_level_inherited: false,
                                  }));
                                }}
                              />
                            </div>
                          </div>
                        </div>
                      ) : null}

                      {modSubTab === "core" ? (
                        <>
                          <p className="muted small" style={{ margin: "0 0 12px" }}>
                            核心规则引擎按攻击类型加载 CRS 规则，支持模块开关、灵敏度与单规则裁剪。
                          </p>
                          {!modulesReadonly && crsStatus ? (
                            <AppCrsVersionBar
                              siteId={site.id}
                              status={crsStatus}
                              onStatusChange={setCrsStatus}
                              onApplied={handleCrsApplied}
                            />
                          ) : null}
                          <CoreModuleSections
                            attack={attackEntries}
                            preAccess={[]}
                            response={[]}
                            readonly={modulesReadonly}
                            configScope="app"
                            onChange={(key, patch) => updateModule(key, patch)}
                            onOpenRules={(key) => {
                              const cat = MODULE_CRS_CATEGORY[key];
                              if (!cat) return;
                              const cfg = modules.modules[key] ?? {
                                enabled: false,
                                action: "block",
                                sensitivity: "medium",
                              };
                              setRulesModal({
                                category: cat,
                                protect: sensitivityToProtectLevel(cfg.sensitivity),
                                title: MODULE_META[key]?.title ?? key,
                                key,
                                disabled: cfg.disabled_rules ?? [],
                              });
                            }}
                          />
                        </>
                      ) : null}

                      {modSubTab === "custom" ? (
                        <div className="module-grid app-config-mod-grid">
                          <p className="muted small" style={{ gridColumn: "1 / -1", margin: "0 0 8px" }}>
                            扩展引擎：挂载签名规则集或插件，补充专家/核心引擎未覆盖的业务场景。
                          </p>
                          {customEntries.length === 0 ? (
                            <NerveEmpty
                              className="ant-empty-sm mod-ext-empty"
                              description={
                                modulesReadonly
                                  ? "可在「防护策略」挂载后，点内置策略卡片上的同步图标写入本应用。"
                                  : "暂无规则集"
                              }
                            />
                          ) : (
                            customEntries.map(({ key, cfg, meta }) => {
                              const setId = key.startsWith(CUSTOM_MODULE_KEY_PREFIX)
                                ? key.slice(CUSTOM_MODULE_KEY_PREFIX.length)
                                : "";
                              return (
                                <ModuleCard
                                  key={key}
                                  entryKey={key}
                                  title={meta?.title ?? key}
                                  desc={meta?.desc ?? ""}
                                  cfg={cfg}
                                  readonly={modulesReadonly}
                                  customSetId={setId}
                                  onChange={(patch) => updateModule(key, patch)}
                                  onOpenRules={() =>
                                    setCustomRulesModal({
                                      setId,
                                      setName: meta?.title ?? setId,
                                    })
                                  }
                                />
                              );
                            })
                          )}
                        </div>
                      ) : null}

                      {modSubTab === "llm_app" ? (
                        <AiProtectionPanel
                          cfg={semantic}
                          readonly={modulesReadonly}
                          onChange={setSemantic}
                        />
                      ) : null}
                    </>
                  )}
                </div>
              </>
            ) : null}

            {!loading && tab === "pipeline" ? (
              <div className="form-stack pipeline-tab-stack">
                <div className="pipeline-editor-section">
                  <div className="pipeline-editor-head">
                    <div>
                      <label className="ant-form-label" style={{ margin: 0 }}>
                        检测流水线
                      </label>
                      <p className="muted small" style={{ margin: "6px 0 0" }}>
                        {PIPELINE_CAPABILITY_INTRO}
                      </p>
                    </div>
                    <div className="pipeline-mode-switch">
                      <button
                        type="button"
                        className={`pipeline-mode-btn${pipelineEditorMode === "list" ? " active" : ""}`}
                        onClick={() => switchPipelineMode("list")}
                      >
                        列表模式
                      </button>
                      <button
                        type="button"
                        className={`pipeline-mode-btn${pipelineEditorMode === "flow" ? " active" : ""}`}
                        onClick={() => switchPipelineMode("flow")}
                      >
                        编排模式
                      </button>
                    </div>
                  </div>
                  {pipelineEditorMode === "list" ? (
                    <div id="pipeline-panel-list" className="pipeline-panel">
                      <PipelineList
                        order={pipelineOrder}
                        enabled={pipelineEnabled}
                        onChange={(order, enabled) =>
                          setSemantic((s) => ({
                            ...s,
                            engines: {
                              ...s.engines,
                              pipeline_order: order,
                              pipeline_enabled: enabled,
                            },
                          }))
                        }
                      />
                    </div>
                  ) : (
                    <div id="pipeline-panel-flow" className="pipeline-panel">
                      <PipelineFlowEditor
                        ref={flowEditorRef}
                        order={pipelineOrder}
                        editor={pipelineEditor}
                        enabled={pipelineEnabled}
                      />
                    </div>
                  )}
                </div>
              </div>
            ) : null}
          </div>

          <div className="app-config-foot">
            <span />
            <div>
              <NerveButton onClick={onClose}>取消</NerveButton>
              <NerveButton
                variant="primary"
                disabled={saving || loading}
                onClick={() => void saveAll()}
              >
                {saving ? "保存中…" : "保存并生效"}
              </NerveButton>
            </div>
          </div>
        </div>
      </div>

      <NerveModal
        open={promptOpen}
        title={promptMode === "create" ? "新建本应用策略" : "编辑本应用策略"}
        onClose={() => setPromptOpen(false)}
        footer={
          <>
            <NerveButton onClick={() => setPromptOpen(false)}>取消</NerveButton>
            <NerveButton variant="primary" onClick={() => void submitPrompt()}>
              {promptMode === "create" ? "创建" : "保存"}
            </NerveButton>
          </>
        }
      >
        <NerveForm>
          <NerveFormItem label="名称" required>
            <NerveInput
              wide
              value={promptName}
              placeholder="请输入名称"
              onChange={(e) => setPromptName(e.target.value)}
            />
          </NerveFormItem>
          <NerveFormItem label="说明">
            <NerveInput
              wide
              value={promptDesc}
              placeholder="可选"
              onChange={(e) => setPromptDesc(e.target.value)}
            />
          </NerveFormItem>
        </NerveForm>
      </NerveModal>

      {basicDetailKey ? (
        <BasicModuleDetailModal
          open
          moduleKey={basicDetailKey}
          cfg={
            modules.modules[basicDetailKey] ?? {
              enabled: false,
              action: "block",
              sensitivity: "medium",
            }
          }
          configScope="app"
          readonly={modulesReadonly}
          onClose={() => setBasicDetailKey(null)}
          onChange={(patch) => updateModule(basicDetailKey, patch)}
        />
      ) : null}

      {rulesModal ? (
        <ModuleRulesModal
          open
          category={rulesModal.category}
          protect={rulesModal.protect}
          moduleTitle={rulesModal.title}
          moduleKey={rulesModal.key}
          moduleEnabled={modules.modules[rulesModal.key]?.enabled !== false}
          editable={!modulesReadonly}
          disabledRules={rulesModal.disabled}
          siteId={site.id}
          onClose={() => setRulesModal(null)}
          onSave={(disabledRules) => {
            updateModule(rulesModal.key, {
              disabled_rules: disabledRules.length ? disabledRules : undefined,
            });
          }}
        />
      ) : null}

      {customRulesModal ? (
        <CustomSetRulesModal
          open
          setId={customRulesModal.setId}
          setName={customRulesModal.setName}
          siteId={!modulesReadonly ? site.id : undefined}
          editable={!modulesReadonly}
          onClose={() => setCustomRulesModal(null)}
        />
      ) : null}
    </>,
    document.body,
  );
}
