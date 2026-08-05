import { useCallback, useEffect, useState } from "react";
import { apiGet, apiPost, apiPut } from "../../api";
import type { ProtectedSite } from "../../sites";
import type {
  DnsCheckResponse,
  GatewayBindingsResponse,
  IngressSaveResponse,
  IngressUpdateBody,
  PlatformIngress,
  SiteIngress,
} from "./ingress-types";
import {
  defaultPlatformIngress,
  isIngressApi404,
  siteToIngress,
} from "./ingress-types";
import type { UpstreamsResponse, WafUpstream } from "../upstreams/upstream-types";

export function useIngress(site: ProtectedSite | null, open: boolean) {
  const siteId = site?.id ?? null;
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ingressApiMissing, setIngressApiMissing] = useState(false);
  const [ingress, setIngress] = useState<SiteIngress | null>(null);
  const [platform, setPlatform] = useState<PlatformIngress | null>(null);
  const [wafUpstreams, setWafUpstreams] = useState<WafUpstream[]>([]);
  const [bindings, setBindings] = useState<GatewayBindingsResponse | null>(null);

  const reload = useCallback(async () => {
    if (!siteId || !site) return;
    setLoading(true);
    setError(null);
    setIngressApiMissing(false);
    try {
      let ing: SiteIngress;
      let api404 = false;
      try {
        ing = await apiGet<SiteIngress>(`/api/sites/${encodeURIComponent(siteId)}/ingress`);
      } catch (e) {
        if (isIngressApi404(e)) {
          ing = siteToIngress(site);
          api404 = true;
        } else {
          throw e;
        }
      }

      let plat: PlatformIngress;
      try {
        plat = await apiGet<PlatformIngress>("/api/ingress/platform");
      } catch (e) {
        if (isIngressApi404(e)) {
          plat = defaultPlatformIngress();
          api404 = true;
        } else {
          throw e;
        }
      }

      const [ups, bind] = await Promise.all([
        apiGet<UpstreamsResponse>("/api/upstreams").catch((e) =>
          isIngressApi404(e) ? { ok: false, upstreams: [] as WafUpstream[] } : Promise.reject(e),
        ),
        apiGet<GatewayBindingsResponse>("/api/sites/gateway-bindings").catch((e) =>
          isIngressApi404(e)
            ? { ok: false, gateway_reachable: false, bindings: {} }
            : Promise.reject(e),
        ),
      ]);

      setIngress(ing);
      setPlatform(plat);
      setWafUpstreams(ups.upstreams ?? []);
      setBindings(bind);
      setIngressApiMissing(api404);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [site, siteId]);

  useEffect(() => {
    if (open && siteId) void reload();
  }, [open, siteId, reload]);

  const saveIngress = useCallback(
    async (body: IngressUpdateBody) => {
      if (!siteId) throw new Error("缺少 site_id");
      if (ingressApiMissing) {
        throw new Error("回源 API 不可用，请先重新编译并重启 nerve-waf-api");
      }
      const res = await apiPut<IngressSaveResponse>(
        `/api/sites/${encodeURIComponent(siteId)}/ingress`,
        body,
      );
      setIngress(res.ingress);
      return res;
    },
    [siteId, ingressApiMissing],
  );

  const dnsCheck = useCallback(async () => {
    if (!siteId) throw new Error("缺少 site_id");
    if (ingressApiMissing) {
      throw new Error("回源 API 不可用，请先重新编译并重启 nerve-waf-api");
    }
    const res = await apiPost<DnsCheckResponse>(
      `/api/sites/${encodeURIComponent(siteId)}/ingress/dns-check`,
    );
    setIngress(res.ingress);
    return res;
  }, [siteId, ingressApiMissing]);

  const applyGateway = useCallback(async () => {
    if (!siteId) throw new Error("缺少 site_id");
    if (ingressApiMissing) {
      throw new Error("回源 API 不可用，请先重新编译并重启 nerve-waf-api");
    }
    return apiPost<{ ok?: boolean; error?: string }>(
      `/api/sites/${encodeURIComponent(siteId)}/ingress/apply-gateway`,
    );
  }, [siteId, ingressApiMissing]);

  const refreshUpstreams = useCallback(async () => {
    const ups = await apiGet<UpstreamsResponse>("/api/upstreams");
    setWafUpstreams(ups.upstreams ?? []);
    return ups;
  }, []);

  return {
    loading,
    error,
    ingressApiMissing,
    ingress,
    platform,
    wafUpstreams,
    bindings,
    reload,
    saveIngress,
    dnsCheck,
    applyGateway,
    refreshUpstreams,
    setIngress,
  };
}
