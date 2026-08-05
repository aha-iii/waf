import { NerveCard } from "@nerve/ui";
import { ingressModeLabel } from "./ingress-display";
import type { IngressMode, IngressOrigin, SiteIngress } from "./ingress-types";

type Props = {
  mode: IngressMode;
  ingress: SiteIngress;
  origins: IngressOrigin[];
  gatewayUpstreamId: string;
};

export function IngressApplySummary({ mode, ingress, gatewayUpstreamId }: Props) {
  return (
    <NerveCard title="摘要">
      <ul className="origin-summary-list">
        <li>
          <span className="muted small">回源</span>
          <span>{ingressModeLabel(mode)}</span>
        </li>
        <li>
          <span className="muted small">域名</span>
          <span>{ingress.hosts.filter((h) => h !== "*").join(", ") || "—"}</span>
        </li>
        <li>
          <span className="muted small">Upstream</span>
          <span>{gatewayUpstreamId || "—"}</span>
        </li>
      </ul>
    </NerveCard>
  );
}
