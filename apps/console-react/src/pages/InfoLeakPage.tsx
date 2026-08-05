import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NervePageHeader,
  NerveTableToolbar,
} from "@nerve/ui";
import { apiGet } from "../api";
import { CoreModuleSection } from "../features/modules/CoreModuleCard";
import { basicModuleDetailSupported } from "../features/modules/basic-module-detail";
import { BasicModuleDetailModal } from "../features/modules/BasicModuleDetailModal";
import { useGlobalModulesAutosave } from "../features/modules/useGlobalModulesAutosave";
import { orderedResponseModuleEntries, type ModuleCfg } from "../module-meta";
import { PAGE_META } from "../nav";
import type { ProtectionModules } from "../lib/waf-types";

export function InfoLeakPage() {
  const meta = PAGE_META["info-leak"];
  const [loading, setLoading] = useState(true);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState("block");
  const [version, setVersion] = useState(1);
  const [modules, setModules] = useState<Record<string, ModuleCfg>>({});
  const [extraModules, setExtraModules] = useState<Record<string, ModuleCfg>>({});
  const [detailOpen, setDetailOpen] = useState(false);

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

  const responseRows = useMemo(() => orderedResponseModuleEntries(modules), [modules]);

  const updateMod = (key: string, patch: Partial<ModuleCfg>) => {
    setModules((prev) => {
      const cur = prev[key] ?? { enabled: false, action: "block", sensitivity: "medium" };
      return { ...prev, [key]: { ...cur, ...patch } };
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
        {saveHint ? <span className="muted small">{saveHint}</span> : null}
      </NerveTableToolbar>

      {error || autosave.error ? (
        <NerveAlert tone="error">{error ?? autosave.error}</NerveAlert>
      ) : null}
      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading && !error ? (
        <CoreModuleSection
          entries={responseRows}
          configScope="global"
          onChange={updateMod}
          onOpenDetail={(key) => {
            if (basicModuleDetailSupported(key)) setDetailOpen(true);
          }}
        />
      ) : null}

      {detailOpen && modules.info_leak ? (
        <BasicModuleDetailModal
          open
          moduleKey="info_leak"
          cfg={modules.info_leak}
          configScope="global"
          onClose={() => setDetailOpen(false)}
          onChange={(patch) => updateMod("info_leak", patch)}
        />
      ) : null}
    </>
  );
}
