import { NerveFormItem, NerveSelect } from "@nerve/ui";
import type { WafUpstream } from "../upstreams/upstream-types";
import { upstreamOriginKind, upstreamOriginKindLabel } from "../upstreams/upstream-types";

type Props = {
  upstreams: WafUpstream[];
  selectedUpstreamId: string;
  onUpstreamChange: (id: string) => void;
};

export function UpstreamSelectPanel({
  upstreams,
  selectedUpstreamId,
  onUpstreamChange,
}: Props) {
  return (
    <NerveFormItem label="Upstream">
      <NerveSelect
        wide
        value={selectedUpstreamId}
        onChange={(e) => onUpstreamChange(e.target.value)}
        options={[
          { value: "", label: "请选择" },
          ...upstreams.map((u) => ({
            value: u.id,
            label: `${u.id} · ${upstreamOriginKindLabel(upstreamOriginKind(u))} · ${u.scheme?.toUpperCase() ?? "HTTP"}`,
          })),
        ]}
      />
    </NerveFormItem>
  );
}
