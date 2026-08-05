/** UI：WAF 边缘回源 | Gateway 穿透 */
export type IngressMode = "waf-edge" | "nerve-gateway";

export type IngressOrigin = {
  address: string;
  port: number;
  scheme: string;
  host_header?: string;
  sni?: string;
  weight: number;
  backup: boolean;
  hc_type: string;
  hc_path?: string;
  hc_interval_secs: number;
};

export type SiteIngress = {
  site_id: string;
  ingress_mode: string;
  cname_slug?: string;
  cname_fqdn?: string;
  dns_status: string;
  dns_checked_at?: string;
  dns_error?: string;
  tls_mode: string;
  certificate_id?: string;
  websocket_enabled: boolean;
  xff_trusted_hops: number;
  origins: IngressOrigin[];
  edge_pop_ids: string[];
  gateway_upstream_id?: string;
  hosts: string[];
  has_upload_cert?: boolean;
  gateway_dashboard_url?: string;
};

export type PlatformIngress = {
  cname_zone: string;
  edge_ips: string[];
  egress_cidrs: string[];
  acme_enabled: boolean;
  hint?: string;
};

export type GatewayUpstream = {
  id: string;
  name: string;
  enabled: boolean;
  scheme: string;
  type: string;
  nodes: { host: string; port: number; weight?: number }[];
  description?: string;
};

export type GatewayUpstreamsResponse = {
  ok: boolean;
  upstreams: GatewayUpstream[];
  admin_url?: string;
  dashboard_url?: string;
  error?: string;
  hint?: string;
};

export type GatewayBindingsResponse = {
  ok: boolean;
  gateway_reachable: boolean;
  bindings: Record<
    string,
    {
      mounted: boolean;
      route_count: number;
      hosts_covered: string[];
      hosts_uncovered: string[];
    }
  >;
  dashboard_url?: string;
  admin_url?: string;
  error?: string;
};

export type IngressUpdateBody = {
  ingress_mode?: string;
  cname_slug?: string;
  tls_mode?: string;
  websocket_enabled?: boolean;
  xff_trusted_hops?: number;
  origins?: IngressOrigin[];
  edge_pop_ids?: string[];
  gateway_upstream_id?: string;
  clear_gateway_upstream?: boolean;
  tls_cert?: string;
  tls_key?: string;
  apply_gateway?: boolean;
};

export type IngressSaveResponse = {
  ok: boolean;
  ingress: SiteIngress;
  gateway_apply?: { ok?: boolean; error?: string };
};

export type DnsCheckResponse = {
  ok: boolean;
  ingress: SiteIngress;
  checked_ok: string[];
  errors: string[];
};

export const INGRESS_STEPS = [
  "回源方式",
  "回源配置",
  "高级选项",
  "保存并生效",
] as const;

export function defaultOrigin(): IngressOrigin {
  return {
    address: "",
    port: 80,
    scheme: "http",
    weight: 1,
    backup: false,
    hc_type: "tcp",
    hc_interval_secs: 10,
  };
}

/** upstream 级参数（UI 从首条 origin 读写，保存时同步到全部节点） */
export type UpstreamSettings = {
  scheme: string;
  host_header?: string;
  sni?: string;
  hc_type: string;
  hc_path?: string;
  hc_interval_secs: number;
};

export type UpstreamNode = {
  address: string;
  port: number;
  weight: number;
};

export function upstreamSettingsFromOrigins(origins: IngressOrigin[]): UpstreamSettings {
  const first = origins[0] ?? defaultOrigin();
  return {
    scheme: first.scheme,
    host_header: first.host_header,
    sni: first.sni,
    hc_type: first.hc_type,
    hc_path: first.hc_path,
    hc_interval_secs: first.hc_interval_secs,
  };
}

export function nodesFromOrigins(origins: IngressOrigin[]): UpstreamNode[] {
  const list = origins.length ? origins : [defaultOrigin()];
  return list.map((o) => ({
    address: o.address,
    port: o.port,
    weight: o.weight,
  }));
}

export function mergeUpstreamOrigins(
  settings: UpstreamSettings,
  nodes: UpstreamNode[],
): IngressOrigin[] {
  const rows = nodes.length ? nodes : [{ address: "", port: settings.scheme === "https" ? 443 : 80, weight: 1 }];
  return rows.map((n) => ({
    ...defaultOrigin(),
    ...settings,
    address: n.address,
    port: n.port,
    weight: Math.max(1, n.weight || 1),
    backup: false,
  }));
}

export function normalizeIngressMode(raw?: string): IngressMode {
  const m = (raw ?? "waf-edge").trim().toLowerCase();
  if (m === "nerve-gateway" || m === "gateway-bypass") return "nerve-gateway";
  return "waf-edge";
}

export function defaultPlatformIngress(): PlatformIngress {
  return {
    cname_zone: "waf.local",
    edge_ips: ["127.0.0.1"],
    egress_cidrs: [],
    acme_enabled: true,
    hint: "将业务域名 CNAME 到应用的 cname_fqdn",
  };
}

/** 后端 ingress API 不可用时，从 /api/sites 条目构造初始视图 */
export function siteToIngress(site: {
  id: string;
  hosts: string[];
  ingress_mode?: string;
  integration?: string;
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
}): SiteIngress {
  const slug = site.cname_slug?.trim() || site.id;
  return {
    site_id: site.id,
    ingress_mode: normalizeIngressMode(site.ingress_mode ?? site.integration),
    cname_slug: slug,
    cname_fqdn: site.cname_fqdn,
    dns_status: site.dns_status ?? "pending",
    dns_checked_at: site.dns_checked_at,
    dns_error: site.dns_error,
    tls_mode: site.tls_mode ?? "auto",
    websocket_enabled: site.websocket_enabled ?? false,
    xff_trusted_hops: site.xff_trusted_hops ?? 1,
    origins: site.origins?.length ? site.origins.map((o) => ({ ...o })) : [],
    edge_pop_ids: [],
    gateway_upstream_id: site.gateway_upstream_id,
    hosts: site.hosts,
  };
}

export function isIngressApi404(err: unknown): boolean {
  return err instanceof Error && /\bHTTP 404\b/.test(err.message);
}
