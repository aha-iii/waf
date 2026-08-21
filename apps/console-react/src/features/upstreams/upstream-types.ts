/** upstreams feature: Upstream types. */
export type UpstreamNode = {
  host: string;
  port: number;
  weight: number;
};

export type UpstreamTransport = {
  connect_timeout_sec?: number;
  send_timeout_sec?: number;
  receive_timeout_sec?: number;
  retry_count?: number;
  retry_timeout_sec?: number;
  pool_capacity?: number;
  pool_idle_timeout_sec?: number;
  pool_max_requests?: number;
};

/** Active + passive health-check config */
export type ActiveHealthyThreshold = {
  interval: number;
  successes: number;
  http_statuses: number[];
};

export type ActiveUnhealthyThreshold = {
  interval: number;
  http_failures: number;
  tcp_failures: number;
  timeouts: number;
  http_statuses: number[];
};

export type PassiveHealthyThreshold = {
  successes: number;
  http_statuses: number[];
};

export type PassiveUnhealthyThreshold = {
  http_failures: number;
  tcp_failures: number;
  timeouts: number;
  http_statuses: number[];
};

export type ActiveHealthCheck = {
  type: string;
  timeout: number;
  concurrency?: number;
  http_path: string;
  host?: string;
  port?: number;
  https_verify_certificate?: boolean;
  healthy: ActiveHealthyThreshold;
  unhealthy: ActiveUnhealthyThreshold;
};

export type PassiveHealthCheck = {
  type: string;
  healthy: PassiveHealthyThreshold;
  unhealthy: PassiveUnhealthyThreshold;
};

export type UpstreamChecks = {
  active?: ActiveHealthCheck | null;
  passive?: PassiveHealthCheck | null;
};

/** @deprecated legacy single-block config; migrated to checks on load */
export type UpstreamHealthCheck = {
  enabled: boolean;
  hc_type: string;
  path: string;
  interval_sec: number;
  timeout_sec?: number;
  healthy_successes?: number;
  unhealthy_timeouts?: number;
  healthy_statuses?: number[];
  unhealthy_statuses?: number[];
};

export type UpstreamOriginKind = "direct" | "gateway";

export type WafUpstream = {
  id: string;
  name: string;
  enabled: boolean;
  /** Node origin | Gateway pass-through */
  origin_kind?: UpstreamOriginKind;
  scheme: string;
  type: string;
  nodes: UpstreamNode[];
  description?: string;
  host_header?: string;
  sni?: string;
  transport?: UpstreamTransport;
  checks?: UpstreamChecks;
  health_check?: UpstreamHealthCheck;
};

export type UpstreamsResponse = {
  ok: boolean;
  upstreams: WafUpstream[];
  gateway_sync?: { ok?: boolean; error?: string };
};

const ACTIVE_HEALTHY_STATUSES = [200, 302];
const ACTIVE_UNHEALTHY_STATUSES = [429, 404, 500, 501, 502, 503, 504, 505];
const PASSIVE_HEALTHY_STATUSES = [
  200, 201, 202, 203, 204, 205, 206, 207, 208, 226, 300, 301, 302, 303, 304, 305, 306, 307, 308,
];
const PASSIVE_UNHEALTHY_STATUSES = [429, 500, 503];

export function defaultActiveCheck(): ActiveHealthCheck {
  return {
    type: "http",
    timeout: 1,
    concurrency: 10,
    http_path: "/",
    https_verify_certificate: true,
    healthy: {
      interval: 10,
      successes: 2,
      http_statuses: [...ACTIVE_HEALTHY_STATUSES],
    },
    unhealthy: {
      interval: 10,
      http_failures: 5,
      tcp_failures: 2,
      timeouts: 3,
      http_statuses: [...ACTIVE_UNHEALTHY_STATUSES],
    },
  };
}

export function defaultPassiveCheck(): PassiveHealthCheck {
  return {
    type: "http",
    healthy: {
      successes: 5,
      http_statuses: [...PASSIVE_HEALTHY_STATUSES],
    },
    unhealthy: {
      http_failures: 3,
      tcp_failures: 2,
      timeouts: 7,
      http_statuses: [...PASSIVE_UNHEALTHY_STATUSES],
    },
  };
}

export function defaultChecks(): UpstreamChecks {
  return {
    active: defaultActiveCheck(),
    passive: defaultPassiveCheck(),
  };
}

export function legacyHealthCheckToChecks(hc: UpstreamHealthCheck): UpstreamChecks {
  return {
    active: {
      type: hc.hc_type || "http",
      timeout: hc.timeout_sec ?? 1,
      http_path: hc.path || "/",
      healthy: {
        interval: hc.interval_sec || 10,
        successes: hc.healthy_successes ?? 2,
        http_statuses: hc.healthy_statuses ?? [...ACTIVE_HEALTHY_STATUSES],
      },
      unhealthy: {
        interval: hc.interval_sec || 10,
        http_failures: 5,
        tcp_failures: 2,
        timeouts: hc.unhealthy_timeouts ?? 3,
        http_statuses: [...ACTIVE_UNHEALTHY_STATUSES],
      },
    },
    passive: defaultPassiveCheck(),
  };
}

export function normalizeUpstream(u: WafUpstream): WafUpstream {
  const withKind: WafUpstream = {
    ...u,
    origin_kind: u.origin_kind === "gateway" ? "gateway" : "direct",
  };
  if (withKind.checks?.active || withKind.checks?.passive) {
    const { health_check: _, ...rest } = withKind;
    return rest;
  }
  if (withKind.health_check?.enabled) {
    const { health_check: hc, ...rest } = withKind;
    return { ...rest, checks: legacyHealthCheckToChecks(hc) };
  }
  return withKind;
}

export function checksEnabled(checks?: UpstreamChecks): boolean {
  return Boolean(checks?.active || checks?.passive);
}

export function statusListToString(list: number[] | undefined): string {
  return (list ?? []).join(", ");
}

export function parseStatusList(raw: string, fallback: number[]): number[] {
  const items = raw
    .split(/[,，\s]+/)
    .map((s) => Number(s.trim()))
    .filter((n) => Number.isFinite(n) && n >= 100 && n <= 599);
  return items.length ? items : fallback;
}

export function defaultUpstream(): WafUpstream {
  return {
    id: `up-${Date.now().toString(36)}`,
    name: "",
    enabled: true,
    origin_kind: "direct",
    scheme: "http",
    type: "roundrobin",
    nodes: [{ host: "", port: 80, weight: 1 }],
    transport: {
      connect_timeout_sec: 300,
      send_timeout_sec: 300,
      receive_timeout_sec: 300,
      retry_count: 3,
      retry_timeout_sec: 60,
      pool_capacity: 320,
      pool_idle_timeout_sec: 60,
      pool_max_requests: 1000,
    },
  };
}

export function nodesSummary(nodes: UpstreamNode[]): string {
  const valid = nodes.filter((n) => n.host.trim());
  if (!valid.length) return "—";
  return valid.map((n) => `${n.host}:${n.port}`).join(", ");
}

export function upstreamOriginKind(u?: WafUpstream | null): UpstreamOriginKind {
  return u?.origin_kind === "gateway" ? "gateway" : "direct";
}

export function upstreamOriginKindLabel(kind: UpstreamOriginKind): string {
  return kind === "gateway" ? "Gateway 穿透" : "节点回源";
}

export function ingressModeFromUpstream(u?: WafUpstream | null): "waf-edge" | "nerve-gateway" {
  return upstreamOriginKind(u) === "gateway" ? "nerve-gateway" : "waf-edge";
}
