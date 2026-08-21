/** ingress feature: Ingress steps. */
import type { UpstreamOriginKind } from "../upstreams/upstream-types";

export function ingressStepsForUpstreamKind(_kind: UpstreamOriginKind): readonly string[] {
  return ["回源配置", "保存并生效"] as const;
}

export function isOriginConfirmStep(_kind: UpstreamOriginKind, step: number): boolean {
  return step === 1;
}
