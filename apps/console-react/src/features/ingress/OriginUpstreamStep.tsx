/** ingress feature: Origin Upstream Step. */
import { UpstreamSelectPanel } from "./UpstreamSelectPanel";
import type { WafUpstream } from "../upstreams/upstream-types";

type Props = {
  upstreams: WafUpstream[];
  selectedUpstreamId: string;
  onUpstreamIdChange: (id: string) => void;
};

export function OriginUpstreamStep({
  upstreams,
  selectedUpstreamId,
  onUpstreamIdChange,
}: Props) {
  return (
    <UpstreamSelectPanel
      upstreams={upstreams}
      selectedUpstreamId={selectedUpstreamId}
      onUpstreamChange={onUpstreamIdChange}
    />
  );
}
