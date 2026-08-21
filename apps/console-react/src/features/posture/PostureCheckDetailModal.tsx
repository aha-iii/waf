/** posture feature: Posture Check Detail Modal. */
import { NerveModal, NerveTable } from "@nerve/ui";
import { categoryLabel } from "../../lib/labels";
import { moduleShortLabel } from "../../lib/module-display";
import { siteDisplayName, type ProtectedSite } from "../../sites";

export type PostureCheckItem = {
  kind: string;
  module?: string;
  rule_id?: string;
  label?: string;
  value?: string;
  count?: number;
  text?: string;
  status?: string;
};

export type PostureCheckRow = {
  site_id: string;
  label: string;
  status: string;
  detail: string;
  items?: PostureCheckItem[];
};

type Props = {
  open: boolean;
  check: PostureCheckRow | null;
  sites: ProtectedSite[];
  onClose: () => void;
};

function statusLabel(status: string): string {
  if (status === "pass") return "通过";
  if (status === "fail") return "未通过";
  if (status === "warn") return "关注";
  if (status === "skip") return "跳过";
  return status;
}

export function PostureCheckDetailModal({ open, check, sites, onClose }: Props) {
  if (!check) return null;

  const items = check.items ?? [];
  const disabledRules = items.filter((i) => i.kind === "disabled_rule");
  const fpStats = items.filter((i) => i.kind === "stat");
  const fpRules = items.filter((i) => i.kind === "fp_rule");
  const hints = items.filter((i) => i.kind === "hint");
  const modules = items.filter((i) => i.kind === "module");

  return (
    <NerveModal
      open={open}
      title={check.label}
      onClose={onClose}
      xl
      footer={null}
    >
      <p className="muted small" style={{ marginTop: 0 }}>
        应用：{siteDisplayName(sites, check.site_id)} · 结果：{statusLabel(check.status)} ·{" "}
        {check.detail}
      </p>

      {disabledRules.length > 0 ? (
        <>
          <h4 style={{ marginBottom: 8 }}>已禁用规则</h4>
          <NerveTable
            rowKey={(r) => `${r.module}-${r.rule_id}`}
            dataSource={disabledRules}
            columns={[
              {
                key: "module",
                title: "模块",
                render: (r) => moduleShortLabel(r.module ?? "") || categoryLabel(r.module ?? ""),
              },
              {
                key: "rule_id",
                title: "规则 ID",
                render: (r) => <code>{r.rule_id}</code>,
              },
            ]}
            emptyText="无"
          />
        </>
      ) : null}

      {fpStats.length > 0 ? (
        <>
          <h4 style={{ margin: `${disabledRules.length ? 16 : 0}px 0 8px` }}>误报统计（近 7 天）</h4>
          <ul className="small" style={{ margin: 0, paddingLeft: 18 }}>
            {fpStats.map((s) => (
              <li key={s.label}>
                {s.label}：{s.value}
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {fpRules.length > 0 ? (
        <>
          <h4 style={{ margin: "16px 0 8px" }}>误报较多规则</h4>
          <NerveTable
            rowKey="rule_id"
            dataSource={fpRules}
            columns={[
              {
                key: "rule_id",
                title: "规则 ID",
                render: (r) => <code>{r.rule_id}</code>,
              },
              { key: "count", title: "确认误报次数", dataIndex: "count" },
            ]}
            emptyText="无"
          />
        </>
      ) : null}

      {modules.length > 0 ? (
        <>
          <h4 style={{ marginBottom: 8 }}>模块状态</h4>
          <ul className="small" style={{ margin: 0, paddingLeft: 18 }}>
            {modules.map((m) => (
              <li key={m.module}>
                {moduleShortLabel(m.module ?? "") || m.module}：已关闭
              </li>
            ))}
          </ul>
        </>
      ) : null}

      {hints.map((h) => (
        <p key={h.text} className="muted small" style={{ marginTop: 12 }}>
          {h.text}
        </p>
      ))}

      {items.length === 0 ? (
        <p className="muted" style={{ marginTop: 12 }}>
          暂无更多明细。
        </p>
      ) : null}
    </NerveModal>
  );
}
