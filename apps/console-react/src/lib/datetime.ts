/** Console 展示时区；开发/测试默认东八区，可用 VITE_CONSOLE_TIME_ZONE 覆盖。 */
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

/** 完整时间：YYYY-MM-DD HH:mm:ss（指定时区） */
export function formatTs(ts: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  const p = partsInZone(ts, timeZone);
  if (!p) return ts.slice(0, 19).replace("T", " ");
  return `${p.year}-${p.month}-${p.day} ${p.hour}:${p.minute}:${p.second}`;
}

/** 攻击日志表格：YYYY-MM-DD HH:mm:ss（指定时区） */
export function formatTsEvent(ts: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  return formatTs(ts, timeZone);
}

/** 按 Console 时区取小时（0–23），用于趋势图分桶 */
export function hourInConsoleZone(ts: string, timeZone: string = CONSOLE_TIME_ZONE): number {
  const p = partsInZone(ts, timeZone);
  if (!p) return new Date(ts).getHours();
  return Number(p.hour);
}

/** 中文 locale 日期时间（指定时区） */
export function formatDateTimeZh(iso: string, timeZone: string = CONSOLE_TIME_ZONE): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-CN", { timeZone, hour12: false });
}
