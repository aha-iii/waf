import { formatDateTimeZh } from "./datetime";

/** datetime-local 值（本地时区）→ ISO UTC；空则 undefined */
export function localDatetimeToIso(local: string): string | undefined {
  const v = local.trim();
  if (!v) return undefined;
  const d = new Date(v);
  if (Number.isNaN(d.getTime())) return undefined;
  return d.toISOString();
}

/** 默认到期：当前本地时间 + hours 小时，格式适配 datetime-local */
export function defaultExpiresLocal(hours = 24): string {
  const d = new Date(Date.now() + hours * 3600_000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function formatExpiresHint(iso?: string): string {
  if (!iso) return "不设到期（需手动解封）";
  try {
    return formatDateTimeZh(iso);
  } catch {
    return iso;
  }
}
