import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  NerveAlert,
  NerveButton,
  nerveConfirm,
  useToast,
} from "@nerve/ui";
import type { ProtectedSite } from "../../sites";
import {
  ingressModeFromUpstream,
  upstreamOriginKind,
} from "../upstreams/upstream-types";
import { IngressApiMissingBanner } from "./IngressApiMissingBanner";
import { IngressApplySummary } from "./IngressApplySummary";
import {
  ingressStepsForUpstreamKind,
  isOriginConfirmStep,
} from "./ingress-steps";
import { OriginUpstreamStep } from "./OriginUpstreamStep";
import {
  defaultOrigin,

  type IngressOrigin,

  type SiteIngress,

} from "./ingress-types";

import { useIngress } from "./useIngress";



export type OriginConfigDrawerProps = {

  open: boolean;

  site: ProtectedSite;

  onClose: () => void;

  onSaved: () => void;

  onConfigureApp?: () => void;

};



type Draft = {

  ingress: SiteIngress;

  origins: IngressOrigin[];

  gatewayUpstreamId: string;

  tlsCert: string;

  tlsKey: string;

};



function buildDraft(ingress: SiteIngress): Draft {

  const origins = ingress.origins?.length ? ingress.origins.map((o) => ({ ...o })) : [defaultOrigin()];

  return {

    ingress: { ...ingress },

    origins,

    gatewayUpstreamId: ingress.gateway_upstream_id ?? "",

    tlsCert: "",

    tlsKey: "",

  };

}



export function OriginConfigDrawer({

  open,

  site,

  onClose,

  onSaved,

  onConfigureApp,

}: OriginConfigDrawerProps) {
  const toast = useToast();

  const {

    loading,

    error,

    ingress,

    wafUpstreams,

    saveIngress,

    ingressApiMissing,

  } = useIngress(site, open);



  const [step, setStep] = useState(0);

  const [maxVisited, setMaxVisited] = useState(0);

  const [draft, setDraft] = useState<Draft | null>(null);

  const [saving, setSaving] = useState(false);



  const openedRef = useRef(false);



  const selectedUpstream = useMemo(

    () => wafUpstreams.find((u) => u.id === draft?.gatewayUpstreamId) ?? null,

    [wafUpstreams, draft?.gatewayUpstreamId],

  );



  const upstreamKind = upstreamOriginKind(selectedUpstream);



  const steps = useMemo(() => ingressStepsForUpstreamKind(upstreamKind), [upstreamKind]);



  const ingressMode = ingressModeFromUpstream(selectedUpstream);



  useEffect(() => {

    if (!open) {

      openedRef.current = false;

      return;

    }

    if (ingress && !openedRef.current) {

      setDraft(buildDraft(ingress));

      setStep(0);

      setMaxVisited(0);

      openedRef.current = true;

    }

  }, [open, ingress]);



  useEffect(() => {

    const max = steps.length - 1;

    if (step > max) setStep(max);

  }, [steps.length, step]);



  const patchIngress = useCallback((patch: Partial<SiteIngress>) => {

    setDraft((d) => (d ? { ...d, ingress: { ...d.ingress, ...patch } } : d));

  }, []);



  const stepValid = useMemo(() => {

    if (!draft) return false;

    if (step === 0) return Boolean(draft.gatewayUpstreamId.trim());

    return true;

  }, [draft, step]);



  const goNext = () => {

    if (!stepValid) return;

    setStep((s) => {

      const n = Math.min(steps.length - 1, s + 1);

      setMaxVisited((m) => Math.max(m, n));

      return n;

    });

  };



  const goPrev = () => setStep((s) => Math.max(0, s - 1));



  const handleSave = async () => {

    if (!draft) return;

    const ok = await nerveConfirm({

      title: "保存并生效",

      message: "将发布边缘配置，可能影响线上流量。确认继续？",

    });

    if (!ok) return;



    setSaving(true);

    try {

      const body = {

        ingress_mode: ingressMode,

        tls_mode: draft.ingress.tls_mode,

        websocket_enabled: draft.ingress.websocket_enabled,

        xff_trusted_hops: draft.ingress.xff_trusted_hops,

        gateway_upstream_id: draft.gatewayUpstreamId.trim() || undefined,

        clear_gateway_upstream: !draft.gatewayUpstreamId.trim(),

        tls_cert: draft.tlsCert.trim() || undefined,

        tls_key: draft.tlsKey.trim() || undefined,

        apply_gateway: false,

      };



      await saveIngress(body);

      

      onSaved();

      onClose();

    } catch (e) {

      toast(e instanceof Error ? e.message : String(e), false);

    } finally {

      setSaving(false);

    }

  };



  if (!open) return null;



  const wildcardOnly =

    site.id !== "default" &&

    (site.hosts.length === 0 || (site.hosts.length === 1 && site.hosts[0]?.trim() === "*"));



  const stepBody = () => {

    if (!draft) return null;



    if (step === 0) {

      if (wildcardOnly) {

        return (

          <NerveAlert tone="warning">

            请先填写具体防护域名。

            {onConfigureApp ? (

              <>

                {" "}

                <NerveButton variant="link" size="sm" onClick={onConfigureApp}>

                  去配置应用

                </NerveButton>

              </>

            ) : null}

          </NerveAlert>

        );

      }

      return (

        <OriginUpstreamStep
          upstreams={wafUpstreams}
          selectedUpstreamId={draft.gatewayUpstreamId}
          onUpstreamIdChange={(id) => setDraft((d) => (d ? { ...d, gatewayUpstreamId: id } : d))}
        />

      );

    }



    if (isOriginConfirmStep(upstreamKind, step)) {

      return (

        <IngressApplySummary

          mode={ingressMode}

          ingress={draft.ingress}

          origins={draft.origins}

          gatewayUpstreamId={draft.gatewayUpstreamId}

        />

      );

    }



    return null;

  };



  const onLastStep = isOriginConfirmStep(upstreamKind, step);



  return createPortal(

    <div

      className="app-config-overlay"

      onClick={(e) => {

        if (e.target === e.currentTarget) onClose();

      }}

    >

      <div className="app-config-drawer origin-config-drawer" onClick={(e) => e.stopPropagation()}>

        <div className="app-config-head">

          <div>

            <h3>{site.name} · 回源配置</h3>

          </div>

          <NerveButton variant="link" onClick={onClose}>

            ✕

          </NerveButton>

        </div>



        <nav className="origin-step-bar" aria-label="回源配置步骤">

          {steps.map((label, i) => (

            <button

              key={label}

              type="button"

              className={`origin-step${step === i ? " active" : ""}${i < step ? " done" : ""}${i <= maxVisited ? " visited" : ""}`}

              disabled={i > maxVisited}

              onClick={() => i <= maxVisited && setStep(i)}

            >

              <span className="origin-step-num">{i + 1}</span>

              <span className="origin-step-label">{label}</span>

            </button>

          ))}

        </nav>



        <div className="app-config-body origin-config-body">

          {loading ? <div className="origin-loading muted">加载回源配置…</div> : null}

          {ingressApiMissing ? <IngressApiMissingBanner /> : null}

          {error && !ingressApiMissing ? <NerveAlert tone="error">{error}</NerveAlert> : null}

          {!loading && draft ? (

            <div className="origin-step-content">{stepBody()}</div>

          ) : null}

        </div>



        <div className="origin-config-foot">

          <NerveButton onClick={goPrev} disabled={step === 0 || saving}>

            上一步

          </NerveButton>

          {!onLastStep ? (

            <NerveButton variant="primary" onClick={goNext} disabled={!stepValid || loading}>

              下一步

            </NerveButton>

          ) : (

            <NerveButton

              variant="primary"

              onClick={() => void handleSave()}

              disabled={saving || !draft || ingressApiMissing || wildcardOnly || !stepValid}

            >

              {saving ? "保存中…" : "保存并生效"}

            </NerveButton>

          )}

        </div>

      </div>

    </div>,

    document.body,

  );

}


