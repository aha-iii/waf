/** 构建 L7 攻击事件列表的查询参数（服务端分页）。 */
export type EventsListQueryInput = {
  page: number;
  pageSize: number;
  category?: string;
  siteId?: string;
};

export function eventsListOffset(page: number, pageSize: number): number {
  const p = Math.max(1, Math.trunc(page) || 1);
  const size = Math.max(1, Math.trunc(pageSize) || 10);
  return (p - 1) * size;
}

export function buildEventsListQuery(input: EventsListQueryInput): URLSearchParams {
  const pageSize = Math.max(1, Math.trunc(input.pageSize) || 10);
  const q = new URLSearchParams({
    limit: String(pageSize),
    offset: String(eventsListOffset(input.page, pageSize)),
    triage: "true",
    layer: "l7",
  });
  const category = input.category?.trim();
  if (category) q.set("category", category);
  const siteId = input.siteId?.trim();
  if (siteId) q.set("site_id", siteId);
  return q;
}
