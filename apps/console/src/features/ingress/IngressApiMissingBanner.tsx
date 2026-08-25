/** ingress feature: Ingress Api Missing Banner. */
import { NerveAlert } from "@nerve/ui";

export function IngressApiMissingBanner() {
  return (
    <NerveAlert tone="warning" className="origin-api-missing">
      回源 API 未就绪，无法保存。请重新编译 nerve-waf-controller 并重启。
    </NerveAlert>
  );
}
