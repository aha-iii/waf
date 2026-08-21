/** Shared library helpers: Crs runtime. */
import { apiGet } from "../api";

export type CrsEnginePreference = "auto" | "wasm" | "off" | "coraza";

export type CrsCapability = {
  key: string;
  label: string;
  applies: boolean;
  note: string;
};

export type CrsRuntimeStatus = {
  preference: CrsEnginePreference;
  preference_source: string;
  env_override?: string | null;
  effective_engine: "wasm" | "coraza_http" | "libinjection" | "disabled";
  wasm_available: boolean;
  wasm_loaded: boolean;
  wasm_path?: string | null;
  capabilities: CrsCapability[];
};

export async function fetchCrsRuntime(): Promise<CrsRuntimeStatus> {
  return apiGet<CrsRuntimeStatus>("/api/crs/runtime");
}

/** On the expert whole-pack path, core module toggles do not apply to the expert stage */
export function crsModuleControlsLimited(status: CrsRuntimeStatus): boolean {
  return status.effective_engine === "wasm";
}

export function crsRuleTogglesLimited(status: CrsRuntimeStatus): boolean {
  const cap = status.capabilities.find((c) => c.key === "disabled_rules");
  return cap ? !cap.applies : status.effective_engine === "wasm";
}
