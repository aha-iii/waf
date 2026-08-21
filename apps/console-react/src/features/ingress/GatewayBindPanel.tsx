/** ingress feature: Gateway Bind Panel. */
import { NerveButton, NerveFormItem, NerveSelect, NerveTag } from "@nerve/ui";
import { GATEWAY_DASHBOARD_URL } from "../../nav";
import type { GatewayBindingsResponse, GatewayUpstreamsResponse, SiteIngress } from "./ingress-types";

type Props = {
  siteId: string;
  ingress: SiteIngress;
  upstreams: GatewayUpstreamsResponse | null;
  bindings: GatewayBindingsResponse | null;
  selectedUpstreamId: string;
  onUpstreamChange: (id: string) => void;
  onRefreshUpstreams: () => void;
  refreshing: boolean;
};

export function GatewayBindPanel({
  siteId,
  upstreams,
  bindings,
  selectedUpstreamId,
  onUpstreamChange,
  onRefreshUpstreams,
  refreshing,
  ingress,
}: Props) {
  const dashboardUrl =
    upstreams?.dashboard_url || ingress.gateway_dashboard_url || GATEWAY_DASHBOARD_URL;
  const binding = bindings?.bindings?.[siteId];
  const list = upstreams?.upstreams ?? [];
  const selected = list.find((u) => u.id === selectedUpstreamId);

  return (
    <div className="form-stack">
      {binding ? (
        <NerveTag tone={binding.mounted ? "green" : "orange"}>
          Route {binding.mounted ? "已挂载" : "未挂载"}
        </NerveTag>
      ) : null}

      <NerveFormItem label="Upstream">
        <NerveSelect
          wide
          value={selectedUpstreamId}
          onChange={(e) => onUpstreamChange(e.target.value)}
          options={[
            { value: "", label: "请选择" },
            ...list.map((u) => ({
              value: u.id,
              label: `${u.id} · ${u.nodes?.length ?? 0} 节点 · ${u.scheme?.toUpperCase() ?? "HTTP"}`,
            })),
          ]}
        />
      </NerveFormItem>

      <div className="origin-copy-row">
        <NerveButton onClick={onRefreshUpstreams} disabled={refreshing}>
          {refreshing ? "刷新中…" : "刷新"}
        </NerveButton>
        <NerveButton variant="link" onClick={() => window.open(dashboardUrl, "_blank", "noopener,noreferrer")}>
          Gateway Upstreams ↗
        </NerveButton>
      </div>

      {selected ? (
        <div className="origin-upstream-preview muted small">
          {selected.name || selected.id} · {selected.nodes?.length ?? 0} 节点 ·{" "}
          {selected.scheme?.toUpperCase() ?? "HTTP"}
        </div>
      ) : null}
    </div>
  );
}
