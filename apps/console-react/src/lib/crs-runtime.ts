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

/** 专家防护整包路径下，核心模块开关不作用于专家段 */
export function crsModuleControlsLimited(status: CrsRuntimeStatus): boolean {
  return status.effective_engine === "wasm";
}

export function crsRuleTogglesLimited(status: CrsRuntimeStatus): boolean {
  const cap = status.capabilities.find((c) => c.key === "disabled_rules");
  return cap ? !cap.applies : status.effective_engine === "wasm";
}
