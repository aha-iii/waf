/** ingress feature: Ingress display. */
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

/** Whether Console can configure upstream (default site allowed; other * wildcards need a concrete domain first) */
export function siteSupportsOriginConfig(site: ProtectedSite): boolean {
  if (site.id === "default") return true;
  const hosts = site.hosts ?? [];
  if (hosts.length === 1 && hosts[0]?.trim() === "*") return false;
  return true;
}
