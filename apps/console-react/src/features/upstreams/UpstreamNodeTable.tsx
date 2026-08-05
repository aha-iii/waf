import { NerveButton, NerveInput } from "@nerve/ui";
import type { UpstreamNode } from "./upstream-types";

type Props = {
  nodes: UpstreamNode[];
  scheme: string;
  onChange: (nodes: UpstreamNode[]) => void;
};

export function UpstreamNodeTable({ nodes, scheme, onChange }: Props) {
  const list = nodes.length ? nodes : [{ host: "", port: scheme === "https" ? 443 : 80, weight: 1 }];

  const patch = (index: number, patchNode: Partial<UpstreamNode>) => {
    onChange(list.map((n, i) => (i === index ? { ...n, ...patchNode } : n)));
  };

  const add = () => {
    onChange([...list, { host: "", port: scheme === "https" ? 443 : 80, weight: 1 }]);
  };

  const remove = (index: number) => {
    if (list.length <= 1) {
      onChange([{ host: "", port: scheme === "https" ? 443 : 80, weight: 1 }]);
      return;
    }
    onChange(list.filter((_, i) => i !== index));
  };

  return (
    <>
      <div className="upstream-node-toolbar">
        <span className="muted small">至少配置一个有效节点</span>
        <NerveButton variant="primary" size="sm" onClick={add}>
          添加节点
        </NerveButton>
      </div>
      <div className="ant-table-wrapper origin-node-table-wrap upstream-node-table-wrap">
        <table className="ant-table origin-node-table upstream-node-table">
          <thead>
            <tr>
              <th>主机名 / IP</th>
              <th className="origin-col-port">端口</th>
              <th className="origin-col-weight">权重</th>
              <th className="origin-col-action" aria-label="操作" />
            </tr>
          </thead>
          <tbody>
            {list.map((node, index) => (
              <tr key={index}>
                <td>
                  <NerveInput
                    wide
                    value={node.host}
                    placeholder="例如 10.0.0.1 或 origin.example.com"
                    onChange={(e) => patch(index, { host: e.target.value })}
                  />
                </td>
                <td className="origin-col-port">
                  <NerveInput
                    sm
                    type="number"
                    value={String(node.port)}
                    onChange={(e) =>
                      patch(index, {
                        port: Math.max(1, Math.min(65535, Number(e.target.value) || 80)),
                      })
                    }
                  />
                </td>
                <td className="origin-col-weight">
                  <NerveInput
                    sm
                    type="number"
                    value={String(node.weight)}
                    onChange={(e) =>
                      patch(index, { weight: Math.max(1, Number(e.target.value) || 1) })
                    }
                  />
                </td>
                <td className="origin-col-action">
                  <NerveButton
                    variant="link"
                    size="sm"
                    onClick={() => remove(index)}
                    aria-label="删除节点"
                  >
                    删除
                  </NerveButton>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
