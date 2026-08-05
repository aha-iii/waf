import type { IngressMode } from "./ingress-types";
import type { ProtectedSite } from "../../sites";

export function ingressModeLabel(mode: IngressMode): string {
  return mode === "waf-edge" ? "WAF 边缘回源" : "Gateway 穿透";
}

export function ingressModeTagTone(mode: IngressMode): "blue" | "default" {
  return mode === "waf-edge" ? "blue" : "default";
}

export function siteIngressMode(site: ProtectedSite): IngressMode {
  return normalizeSiteMode(site.ingress_mode ?? site.integration);
}

function normalizeSiteMode(raw?: string): IngressMode {
  const m = (raw ?? "waf-edge").trim().toLowerCase();
  if (m === "nerve-gateway" || m === "gateway-bypass") {
    return "nerve-gateway";
  }
  return "waf-edge";
}

/** 是否可在 Console 配置回源（default 站点允许，其余通配 * 需先填具体域名） */
export function siteSupportsOriginConfig(site: ProtectedSite): boolean {
  if (site.id === "default") return true;
  const hosts = site.hosts ?? [];
  if (hosts.length === 1 && hosts[0]?.trim() === "*") return false;
  return true;
}
