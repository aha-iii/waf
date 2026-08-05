import { useCallback, useEffect, useMemo, useState } from "react";
import { NerveAlert, NervePageHeader, NerveSelect, NerveTableToolbar } from "@nerve/ui";
import { apiGet } from "../../api";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "../../crs-preset";
import { CoreModuleSections } from "./CoreModuleCard";
import { basicModuleDetailSupported } from "./basic-module-detail";
import { BasicModuleDetailModal } from "./BasicModuleDetailModal";
import { ModuleRulesModal } from "./ModuleRulesModal";
import { useGlobalModulesAutosave } from "./useGlobalModulesAutosave";
import {
  MODULE_META,
  orderedPreAccessModuleEntries,
  orderedResponseModuleEntries,
  orderedWebAttackModuleEntries,
  type ModuleCfg,
} from "../../module-meta";
import { GLOBAL_DEFAULT_SITE_ID } from "../../sites";
import { PAGE_META, type PageId } from "../../nav";
import type { ProtectionModules } from "../../lib/waf-types";

type RulesModalState = {
  category: string;
  moduleKey: string;
  moduleTitle: string;
  moduleEnabled: boolean;
  protect: 1 | 2 | 3;
};

export type GlobalModulesPageProps = {
  pageId: Extract<PageId, "basic-protection" | "core-protection">;
};

export function GlobalModulesPage({ pageId }: GlobalModulesPageProps) {
  const meta = PAGE_META[pageId];
  const variant = pageId === "basic-protection" ? "basic" : "core";
  const [loading, setLoading] = useState(true);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState("block");
  const [version, setVersion] = useState(1);
  const [modules, setModules] = useState<Record<string, ModuleCfg>>({});
  const [extraModules, setExtraModules] = useState<Record<string, ModuleCfg>>({});
  const [rulesModal, setRulesModal] = useState<RulesModalState | null>(null);
  const [basicDetailKey, setBasicDetailKey] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setReady(false);
    setError(null);
    try {
      const res = await apiGet<ProtectionModules>("/api/global/modules");
      setMode(res.mode);
      setVersion(res.version);
      const extra: Record<string, ModuleCfg> = {};
      for (const key of ["custom", "plugin", "crs"] as const) {
        if (res.modules[key]) extra[key] = { ...res.modules[key] };
      }
      setExtraModules(extra);
      setModules({ ...res.modules });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
      setReady(true);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const autosave = useGlobalModulesAutosave({
    ready,
    mode,
    version,
    modules,
    extraModules,
  });

  const attackRows = useMemo(() => orderedWebAttackModuleEntries(modules), [modules]);
  const preAccessRows = useMemo(() => orderedPreAccessModuleEntries(modules), [modules]);
  const responseRows = useMemo(() => orderedResponseModuleEntries(modules), [modules]);

  const updateMod = (key: string, patch: Partial<ModuleCfg>) => {
    setModules((prev) => {
      const cur = prev[key] ?? { enabled: false, action: "block", sensitivity: "medium" };
      return { ...prev, [key]: { ...cur, ...patch } };
    });
  };

  const openRules = (key: string) => {
    const cat = MODULE_CRS_CATEGORY[key];
    if (!cat) return;
    const cfg = modules[key] ?? { enabled: false, action: "block", sensitivity: "medium" };
    setRulesModal({
      category: cat,
      moduleKey: key,
      moduleTitle: MODULE_META[key]?.title ?? key,
      moduleEnabled: cfg.enabled,
      protect: sensitivityToProtectLevel(cfg.sensitivity),
    });
  };

  const saveHint = autosave.saving
    ? "保存中…"
    : autosave.error
      ? "保存失败"
      : autosave.savedAt
        ? "已保存"
        : null;

  return (
    <>
      <NervePageHeader title={meta.title} />
      <NerveTableToolbar>
        <label className="muted small">
          默认模式{" "}
          <NerveSelect
            value={mode}
            options={[
              { value: "block", label: "拦截" },
              { value: "detect", label: "仅检测" },
            ]}
            style={{ width: 120, marginLeft: 8 }}
            onChange={(e) => setMode(e.target.value)}
          />
        </label>
        {saveHint ? <span className="muted small">{saveHint}</span> : null}
      </NerveTableToolbar>

      {error || autosave.error ? (
        <NerveAlert tone="error">{error ?? autosave.error}</NerveAlert>
      ) : null}
      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading && !error && variant === "basic" ? (
        <CoreModuleSections
          attack={[]}
          preAccess={preAccessRows}
          response={responseRows}
          configScope="global"
          onChange={updateMod}
          onOpenDetail={(key) => {
            if (basicModuleDetailSupported(key)) setBasicDetailKey(key);
          }}
        />
      ) : null}

      {!loading && !error && variant === "core" ? (
        <CoreModuleSections
          attack={attackRows}
          preAccess={[]}
          response={[]}
          configScope="global"
          onChange={updateMod}
          onOpenRules={openRules}
        />
      ) : null}

      {basicDetailKey ? (
        <BasicModuleDetailModal
          open
          moduleKey={basicDetailKey}
          cfg={
            modules[basicDetailKey] ?? { enabled: false, action: "block", sensitivity: "medium" }
          }
          configScope="global"
          onClose={() => setBasicDetailKey(null)}
          onChange={(patch) => updateMod(basicDetailKey, patch)}
        />
      ) : null}

      {rulesModal ? (
        <ModuleRulesModal
          open
          onClose={() => setRulesModal(null)}
          category={rulesModal.category}
          protect={rulesModal.protect}
          moduleTitle={rulesModal.moduleTitle}
          moduleEnabled={rulesModal.moduleEnabled}
          moduleKey={rulesModal.moduleKey}
          siteId={GLOBAL_DEFAULT_SITE_ID}
        />
      ) : null}
    </>
  );
}
