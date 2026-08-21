/** ingress feature: Origin Pool Editor. */
import { NerveButton, NerveFormItem, NerveInput, NerveSelect } from "@nerve/ui";
import type { IngressOrigin } from "./ingress-types";
import {
  defaultOrigin,
  mergeUpstreamOrigins,
  nodesFromOrigins,
  upstreamSettingsFromOrigins,
  type UpstreamNode,
  type UpstreamSettings,
} from "./ingress-types";

type Props = {
  origins: IngressOrigin[];
  onChange: (origins: IngressOrigin[]) => void;
};

function applySchemePortDefaults(scheme: string, nodes: UpstreamNode[]): UpstreamNode[] {
  return nodes.map((n) => {
    if (scheme === "https" && n.port === 80) return { ...n, port: 443 };
    if (scheme === "http" && n.port === 443) return { ...n, port: 80 };
    return n;
  });
}

export function OriginPoolEditor({ origins, onChange }: Props) {
  const list = origins.length ? origins : [defaultOrigin()];
  const settings = upstreamSettingsFromOrigins(list);
  const nodes = nodesFromOrigins(list);

  const emit = (nextSettings: UpstreamSettings, nextNodes: UpstreamNode[]) => {
    onChange(mergeUpstreamOrigins(nextSettings, nextNodes));
  };

  const patchSettings = (patch: Partial<UpstreamSettings>) => {
    let nextNodes = nodes;
    if (patch.scheme && patch.scheme !== settings.scheme) {
      nextNodes = applySchemePortDefaults(patch.scheme, nodes);
    }
    emit({ ...settings, ...patch }, nextNodes);
  };

  const patchNode = (index: number, patch: Partial<UpstreamNode>) => {
    emit(
      settings,
      nodes.map((n, i) => (i === index ? { ...n, ...patch } : n)),
    );
  };

  const addNode = () => {
    const port = settings.scheme === "https" ? 443 : 80;
    emit(settings, [...nodes, { address: "", port, weight: 1 }]);
  };

  const removeNode = (index: number) => {
    if (nodes.length <= 1) {
      emit(settings, [{ address: "", port: settings.scheme === "https" ? 443 : 80, weight: 1 }]);
      return;
    }
    emit(
      settings,
      nodes.filter((_, i) => i !== index),
    );
  };

  return (
    <div className="form-stack origin-upstream-editor">
      <div className="origin-section-head">
        <strong>目标节点</strong>
        <NerveButton variant="primary" size="sm" onClick={addNode}>
          + 新建
        </NerveButton>
      </div>

      <div className="ant-table-wrapper origin-node-table-wrap">
        <table className="ant-table origin-node-table">
          <thead>
            <tr>
              <th>主机名 / IP</th>
              <th className="origin-col-port">端口</th>
              <th className="origin-col-weight">权重</th>
              <th className="origin-col-action" />
            </tr>
          </thead>
          <tbody>
            {nodes.map((node, index) => (
              <tr key={index}>
                <td>
                  <NerveInput
                    wide
                    value={node.address}
                    placeholder="主机名或 IP"
                    onChange={(e) => patchNode(index, { address: e.target.value })}
                  />
                </td>
                <td>
                  <NerveInput
                    type="number"
                    value={String(node.port)}
                    onChange={(e) =>
                      patchNode(index, {
                        port: Math.max(1, Math.min(65535, Number(e.target.value) || 80)),
                      })
                    }
                  />
                </td>
                <td>
                  <NerveInput
                    type="number"
                    value={String(node.weight)}
                    onChange={(e) =>
                      patchNode(index, {
                        weight: Math.max(1, Number(e.target.value) || 1),
                      })
                    }
                  />
                </td>
                <td className="origin-col-action">
                  <NerveButton variant="link" size="sm" onClick={() => removeNode(index)}>
                    −
                  </NerveButton>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="origin-section-head">
        <strong>回源参数</strong>
      </div>
      <div className="origin-pool-fields">
        <NerveFormItem label="Host 请求头">
          <NerveSelect
            wide
            value={settings.host_header !== undefined ? "custom" : "pass"}
            onChange={(e) => {
              if (e.target.value === "pass") {
                patchSettings({ host_header: undefined });
              } else {
                patchSettings({ host_header: settings.host_header ?? "" });
              }
            }}
            options={[
              { value: "pass", label: "与客户端请求一致" },
              { value: "custom", label: "自定义" },
            ]}
          />
        </NerveFormItem>
        {settings.host_header !== undefined ? (
          <NerveFormItem label="自定义 Host">
            <NerveInput
              wide
              value={settings.host_header}
              placeholder="origin.example.com"
              onChange={(e) => patchSettings({ host_header: e.target.value || undefined })}
            />
          </NerveFormItem>
        ) : null}
        <NerveFormItem label="协议">
          <NerveSelect
            value={settings.scheme}
            onChange={(e) => patchSettings({ scheme: e.target.value })}
            options={[
              { value: "http", label: "HTTP" },
              { value: "https", label: "HTTPS" },
            ]}
          />
        </NerveFormItem>
        {settings.scheme === "https" ? (
          <NerveFormItem label="SNI">
            <NerveInput
              wide
              value={settings.sni ?? ""}
              placeholder="留空则使用 Host"
              onChange={(e) => patchSettings({ sni: e.target.value || undefined })}
            />
          </NerveFormItem>
        ) : null}
      </div>

      <details className="origin-pool-advanced">
        <summary className="muted small">健康检查</summary>
        <div className="origin-pool-fields">
          <NerveFormItem label="类型">
            <NerveSelect
              value={settings.hc_type}
              onChange={(e) => patchSettings({ hc_type: e.target.value })}
              options={[
                { value: "tcp", label: "TCP" },
                { value: "http", label: "HTTP" },
                { value: "none", label: "关闭" },
              ]}
            />
          </NerveFormItem>
          {settings.hc_type === "http" ? (
            <NerveFormItem label="请求路径">
              <NerveInput
                wide
                value={settings.hc_path ?? "/"}
                onChange={(e) => patchSettings({ hc_path: e.target.value })}
              />
            </NerveFormItem>
          ) : null}
          {settings.hc_type !== "none" ? (
            <NerveFormItem label="间隔（秒）">
              <NerveInput
                type="number"
                value={String(settings.hc_interval_secs)}
                onChange={(e) =>
                  patchSettings({
                    hc_interval_secs: Math.max(1, Number(e.target.value) || 10),
                  })
                }
              />
            </NerveFormItem>
          ) : null}
        </div>
      </details>
    </div>
  );
}
