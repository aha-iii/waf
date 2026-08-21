/** Site / app selection helpers for multi-tenant console context. */
import { apiGet } from "./api.js";
import type { IngressOrigin } from "./features/ingress/ingress-types";

export type ProtectedSite = {
  id: string;
  name: string;
  hosts: string[];
  upstream?: string;
  enabled: boolean;
  /** Legacy field; detection path does not depend on it; keep on save */
  integration?: string;
  description?: string;
  ingress_mode?: string;
  cname_slug?: string;
  cname_fqdn?: string;
  dns_status?: string;
  dns_checked_at?: string;
  dns_error?: string;
  tls_mode?: string;
  websocket_enabled?: boolean;
  xff_trusted_hops?: number;
  origins?: IngressOrigin[];
  gateway_upstream_id?: string;
};

/** Global default protection template; sidebar WAF settings read/write this site_id; apps inherit when not overridden */
export const GLOBAL_DEFAULT_SITE_ID = "default";

const SITE_KEY = "waf-selected-site";
export const EVENTS_SITE_FILTER_KEY = "nerve-events-site-filter";
export const EVENTS_NAV_EVENT = "nerve-events-nav";

export function setEventsSiteFilter(siteId: string): void {
  sessionStorage.setItem(EVENTS_SITE_FILTER_KEY, siteId);
}

export function consumeEventsSiteFilter(): string | null {
  const v = sessionStorage.getItem(EVENTS_SITE_FILTER_KEY);
  if (v != null) sessionStorage.removeItem(EVENTS_SITE_FILTER_KEY);
  return v;
}

/** Navigate to attack logs with app filter preselected */
export function navigateToAppEvents(siteId: string): void {
  setEventsSiteFilter(siteId);
  const onEvents = location.hash.replace(/^#\/?/, "").split("?")[0] === "events";
  if (onEvents) {
    window.dispatchEvent(new CustomEvent(EVENTS_NAV_EVENT));
  } else {
    location.hash = "#/events";
  }
}

export function getSelectedSiteId(): string {
  return sessionStorage.getItem(SITE_KEY) || "default";
}

export function setSelectedSiteId(id: string): void {
  sessionStorage.setItem(SITE_KEY, id);
}

export function siteQuery(siteId: string): string {
  return `?site_id=${encodeURIComponent(siteId)}`;
}

/** Attack-log and similar: site_id → display name */
export function siteDisplayName(sites: ProtectedSite[], siteId: string | undefined): string {
  const id = (siteId ?? "default").trim() || "default";
  const list = Array.isArray(sites) ? sites : [];
  const hit = list.find((s) => s.id === id);
  if (hit?.name) return hit.name;
  if (id === "default") return "系统默认";
  return id;
}

export async function loadSites(): Promise<ProtectedSite[]> {
  const res = await apiGet<{ sites?: ProtectedSite[] }>("/api/sites");
  const list = Array.isArray(res.sites) ? res.sites : [];
  if (!list.some((s) => s.id === getSelectedSiteId())) {
    setSelectedSiteId(list[0]?.id ?? "default");
  }
  return list;
}
