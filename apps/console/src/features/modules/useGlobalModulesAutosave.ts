/** modules feature: Use Global Modules Autosave. */
import { useEffect, useRef, useState } from "react";
import { apiPut } from "../../api";
import type { ProtectionModules } from "../../lib/waf-types";
import type { ModuleCfg } from "../../module-meta";

type Params = {
  ready: boolean;
  mode: string;
  version: number;
  modules: Record<string, ModuleCfg>;
  extraModules: Record<string, ModuleCfg>;
};

export function useGlobalModulesAutosave({
  ready,
  mode,
  version,
  modules,
  extraModules,
}: Params) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const skipRef = useRef(true);

  useEffect(() => {
    if (!ready) return;
    if (skipRef.current) {
      skipRef.current = false;
      return;
    }

    const timer = setTimeout(async () => {
      setSaving(true);
      setError(null);
      try {
        const payload: ProtectionModules = {
          version,
          mode,
          modules: { ...modules, ...extraModules },
        };
        await apiPut("/api/global/modules", payload);
        setSavedAt(Date.now());
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setSaving(false);
      }
    }, 400);

    return () => clearTimeout(timer);
  }, [ready, mode, version, modules, extraModules]);

  return { saving, error, savedAt };
}
