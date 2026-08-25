/** Console HTTP client for Nerve WAF control-plane APIs. */
import { getAuthHeaders, notifyUnauthorized } from "./auth.js";

const API = "";
const DEFAULT_TIMEOUT_MS = 60_000;

function mergeHeaders(extra?: HeadersInit): HeadersInit {
  return { ...getAuthHeaders(), ...(extra as Record<string, string> | undefined) };
}

async function fetchWithTimeout(
  url: string,
  init: RequestInit,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<Response> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") {
      throw new Error(`请求超时（${Math.round(timeoutMs / 1000)}s），请稍后重试`);
    }
    throw e;
  } finally {
    clearTimeout(timer);
  }
}

async function parseJsonBody<T>(res: Response): Promise<T> {
  const text = await res.text();
  if (!text.trim()) return {} as T;
  return JSON.parse(text) as T;
}

function httpErrorMessage(res: Response, body: string): string {
  const text = body.trim();
  if (text) {
    try {
      const j = JSON.parse(text) as { error?: string; message?: string };
      const msg = j.error || j.message;
      if (msg) return `HTTP ${res.status}: ${msg}`;
    } catch {
      /* plain text */
    }
    return `HTTP ${res.status}: ${text.slice(0, 300)}`;
  }
  return `HTTP ${res.status} ${res.statusText || "Error"}`.trim();
}

function guardUnauthorized(res: Response): void {
  if (res.status === 401) {
    notifyUnauthorized();
    throw new Error("HTTP 401 Unauthorized");
  }
}

export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetchWithTimeout(`${API}${path}`, { headers: mergeHeaders() });
  guardUnauthorized(res);
  if (!res.ok) throw new Error(httpErrorMessage(res, await res.text()));
  return parseJsonBody<T>(res);
}

export async function apiPost<T>(
  path: string,
  body?: unknown,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  const res = await fetchWithTimeout(
    `${API}${path}`,
    {
      method: "POST",
      headers: mergeHeaders(body ? { "Content-Type": "application/json" } : undefined),
      body: body ? JSON.stringify(body) : undefined,
    },
    timeoutMs,
  );
  if (!res.ok) {
    guardUnauthorized(res);
    const text = await res.text();
    let msg = text || res.statusText;
    try {
      const j = JSON.parse(text) as { error?: string; message?: string };
      msg = j.error || j.message || msg;
    } catch {
      /* plain text error body */
    }
    throw new Error(msg);
  }
  return parseJsonBody<T>(res);
}

export async function apiPut<T>(path: string, body: unknown, timeoutMs = DEFAULT_TIMEOUT_MS): Promise<T> {
  const res = await fetchWithTimeout(
    `${API}${path}`,
    {
      method: "PUT",
      headers: mergeHeaders({ "Content-Type": "application/json" }),
      body: JSON.stringify(body),
    },
    timeoutMs,
  );
  guardUnauthorized(res);
  if (!res.ok) throw new Error(httpErrorMessage(res, await res.text()));
  return parseJsonBody<T>(res);
}

export async function apiDelete(path: string): Promise<void> {
  const res = await fetchWithTimeout(`${API}${path}`, { method: "DELETE", headers: mergeHeaders() });
  guardUnauthorized(res);
  if (!res.ok) throw new Error(await res.text());
}

export type L7Match = {
  host?: string;
  path_prefix?: string;
  methods?: string[];
  uri_regex?: string;
};

export type RouteMatch = {
  src_ip?: string;
  dst_ip?: string;
  dst_port?: number;
  pod?: string;
  l7?: L7Match;
};

export type RouteLayer = "l3" | "l7";

export type WafRoute = {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
  layer?: RouteLayer;
  match: RouteMatch;
  plugins: { name: string; enabled: boolean; config: Record<string, unknown> }[];
  upstream_id?: string;
};

export type L7UpstreamNode = {
  host: string;
  port: number;
  weight?: number;
};

export type L7Upstream = {
  id: string;
  name: string;
  enabled: boolean;
  scheme: string;
  type: string;
  nodes: L7UpstreamNode[];
  description?: string;
};

export function inferRouteLayer(route: WafRoute): RouteLayer {
  if (route.layer === "l3" || route.layer === "l7") return route.layer;
  const l7 = route.match.l7;
  if (l7 && (l7.host || l7.path_prefix || l7.methods?.length || l7.uri_regex)) {
    return "l7";
  }
  return "l3";
}

export type WafConfig = { version: number; routes: WafRoute[] };

export type PluginMeta = {
  name: string;
  version: string;
  description: string;
  priority: number;
  layer?: string;
  type?: string;
};

export type SyncResult = {
  ok: boolean;
  applied?: number;
  errors?: string[];
  plan?: unknown;
};

export type RuntimeDenyEntry = {
  ip: string;
  source: string;
  reason?: string;
  expires_at?: string;
  created_at: string;
};

export type TempDenyOptions = {
  source?: string;
  reason?: string;
  /** RFC3339 / ISO string */
  expires_at?: string;
  /** Relative duration (seconds); usable when expires_at is omitted */
  ttl_secs?: number;
};

/** Temporary ban (runtime overlay; does not write plugin config) */
export async function apiTempDeny(
  ip: string,
  sourceOrOpts: string | TempDenyOptions = "console",
): Promise<unknown> {
  const opts: TempDenyOptions =
    typeof sourceOrOpts === "string" ? { source: sourceOrOpts } : sourceOrOpts;
  return apiPost("/api/policy/deny", {
    ip,
    kind: "temporary",
    source: opts.source ?? "console",
    reason: opts.reason || undefined,
    expires_at: opts.expires_at || undefined,
    ttl_secs: opts.ttl_secs,
  });
}

/** Lift temporary ban */
export async function apiTempAllow(ip: string): Promise<unknown> {
  return apiPost("/api/policy/allow", { ip, kind: "temporary" });
}

export async function apiBaselineAllow(pod: string, ip: string): Promise<unknown> {
  const q = new URLSearchParams({ pod, ip });
  return apiPost(`/api/nerve/baseline/allow?${q}`);
}

export async function apiNetworkGeoip(ip: string): Promise<{
  ip: string;
  country?: string;
  country_label?: string;
  source?: string;
}> {
  return apiGet(`/api/network/geoip?ip=${encodeURIComponent(ip)}`);
}

export async function apiListRuntimeDenies(): Promise<{ denies: RuntimeDenyEntry[]; total: number }> {
  return apiGet("/api/runtime-denies");
}

/** Compile from current routes.json and push to Nerve Agent */
export async function apiSync(plan?: unknown): Promise<SyncResult> {
  return apiPost<SyncResult>("/api/sync", plan);
}

export const PLUGIN_DEFAULT_CONFIG: Record<string, Record<string, unknown>> = {
  "ip-blacklist": { ips: ["1.2.3.4"] },
  "ip-whitelist": { ips: [] },
  "port-block": { ports: [4444], deny_dst: true },
  "baseline-allow": { pod: "cgroup:example", ips: ["8.8.8.8"] },
};
