import { apiGet } from "../api";
import { formatDateTimeZh } from "./datetime";
import type { CrsVersion } from "./crs-rules";

export type AppCrsBinding = {
  site_id: string;
  crs_version_id: string;
  crs_version_label?: string;
  rule_count: number;
  applied_at: string;
};

export type AppCrsHistoryEntry = {
  id: string;
  site_id: string;
  crs_version_id: string;
  crs_version_label?: string;
  rule_count: number;
  modules_snapshot: Record<
    string,
    { enabled: boolean; action: string; sensitivity: string; disabled_rules?: string[] }
  >;
  is_active: boolean;
  note?: string;
  applied_at: string;
};

export type AppCrsStatus = {
  binding: AppCrsBinding | null;
  system_latest: {
    crs_version_id: string;
    crs_version_label?: string;
    rule_count: number;
  } | null;
  behind_system: boolean;
};

export function formatVersionLabel(b: {
  crs_version_id: string;
  crs_version_label?: string;
  rule_count: number;
}): string {
  const label = b.crs_version_label ?? b.crs_version_id;
  return `${label} · ${b.crs_version_id} · ${b.rule_count} 条`;
}

export function formatCrsTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : formatDateTimeZh(iso);
}

export async function fetchAppCrsStatus(siteId: string): Promise<AppCrsStatus> {
  return apiGet<AppCrsStatus>(`/api/apps/${encodeURIComponent(siteId)}/crs`);
}

export type { CrsVersion };
