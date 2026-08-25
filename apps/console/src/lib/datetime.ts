/** Console display timezone helpers; default Asia/Shanghai, override via VITE_CONSOLE_TIME_ZONE. */
export const CONSOLE_TIME_ZONE =
  (import.meta.env.VITE_CONSOLE_TIME_ZONE as string | undefined)?.trim() || "Asia/Shanghai";

type ZoneParts = {
  year: string;
  month: string;
  day: string;
  hour: string;
  minute: string;
  second: string;
};

function partsInZone(ts: string, timeZone: string = CONSOLE_TIME_ZONE): ZoneParts | null {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return null;
  const fmt = new Intl.DateTimeFormat("en-CA", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
  const parts = fmt.formatToParts(d);
  const get = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((p) => p.type === type)?.value ?? "";
  return {
    year: get("year"),
    month: get("month"),
    day: get("day"),
    hour: get("hour"),
    minute: get("minute"),
    second: get("second"),
  };
}

/** Full timestamp: YYYY-MM-DD HH:mm:ss (given timezone) */
export function formatTs(ts: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  const p = partsInZone(ts, timeZone);
  if (!p) return ts.slice(0, 19).replace("T", " ");
  return `${p.year}-${p.month}-${p.day} ${p.hour}:${p.minute}:${p.second}`;
}

/** Attack-log table time: YYYY-MM-DD HH:mm:ss (given timezone) */
export function formatTsEvent(ts: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  return formatTs(ts, timeZone);
}

/** Hour-of-day (0–23) in Console timezone for trend chart buckets */
export function hourInConsoleZone(ts: string, timeZone: string = CONSOLE_TIME_ZONE): number {
  const p = partsInZone(ts, timeZone);
  if (!p) return new Date(ts).getHours();
  return Number(p.hour);
}

/** zh-CN locale date-time (given timezone) */
export function formatDateTimeZh(iso: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-CN", { timeZone, hour12: false });
}
