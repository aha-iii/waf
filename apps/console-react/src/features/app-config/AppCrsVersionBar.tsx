import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveModal,
  NerveTable,
  NerveTag,
  nerveConfirm,
  nerveResult,
} from "@nerve/ui";
import { apiGet, apiPost } from "../../api";
import { MODULE_CRS_CATEGORY, sensitivityToProtectLevel } from "../../crs-preset";
import { MODULE_META } from "../../module-meta";
import { ModuleRulesModal } from "../modules/ModuleRulesModal";
import {
  fetchAppCrsStatus,
  formatCrsTime,
  formatVersionLabel,
  type AppCrsHistoryEntry,
  type AppCrsStatus,
  type CrsVersion,
} from "../../lib/app-crs";

export type AppCrsVersionBarProps = {
  siteId: string;
  status: AppCrsStatus | null;
  onStatusChange: (status: AppCrsStatus) => void;
  onApplied: () => void | Promise<void>;
};

export function AppCrsVersionBar({
  siteId,
  status,
  onStatusChange,
  onApplied,
}: AppCrsVersionBarProps) {
  const [applyOpen, setApplyOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [versions, setVersions] = useState<CrsVersion[]>([]);
  const [history, setHistory] = useState<AppCrsHistoryEntry[]>([]);
  const [histConfig, setHistConfig] = useState<AppCrsHistoryEntry | null>(null);
  const [rulesModal, setRulesModal] = useState<{
    category: string;
    protect: ReturnType<typeof sensitivityToProtectLevel>;
    title: string;
    key: string;
    disabled: string[];
  } | null>(null);
  const [loadingApply, setLoadingApply] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);

  const cur = status?.binding
    ? formatVersionLabel(status.binding)
    : "未绑定（跟随系统当前活跃规则集）";
  const sys = status?.system_latest ? formatVersionLabel(status.system_latest) : "—";
  const lagTag = status?.behind_system ? (
    <NerveTag tone="orange">可升级</NerveTag>
  ) : status?.binding ? (
    <NerveTag tone="green">已同步</NerveTag>
  ) : null;

  const openApply = useCallback(async () => {
    setLoadingApply(true);
    try {
      const { versions: vers } = await apiGet<{ versions: CrsVersion[] }>("/api/rules/crs/versions");
      setVersions(vers ?? []);
      setApplyOpen(true);
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "加载失败",
        message: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setLoadingApply(false);
    }
  }, []);

  const openHistory = useCallback(async () => {
    setLoadingHistory(true);
    try {
      const { history: hist } = await apiGet<{ history: AppCrsHistoryEntry[] }>(
        `/api/apps/${encodeURIComponent(siteId)}/crs/history`,
      );
      setHistory(hist ?? []);
      setHistoryOpen(true);
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "加载失败",
        message: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setLoadingHistory(false);
    }
  }, [siteId]);

  const applyVersion = async (ver: string) => {
    const ok = await nerveConfirm({
      title: "应用 CRS 版本",
      message: `将本应用 CRS 切换到版本 ${ver}？\n\n系统全局规则集不会被修改；当前配置将自动存入历史。`,
      okText: "应用",
    });
    if (!ok) return;
    try {
      await apiPost(`/api/apps/${encodeURIComponent(siteId)}/crs/apply`, {
        crs_version_id: ver,
      });
      await nerveResult({
        type: "success",
        title: "应用成功",
        message: `已应用 CRS 版本 ${ver}`,
      });
      setApplyOpen(false);
      onStatusChange(await fetchAppCrsStatus(siteId));
      await onApplied();
    } catch (e) {
      await nerveResult({
        type: "error",
        title: "应用失败",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const openHistRules = (entry: AppCrsHistoryEntry, key: string, viewDisabledOnly: boolean) => {
    const cfg = entry.modules_snapshot[key];
    if (!cfg) return;
    const cat = MODULE_CRS_CATEGORY[key];
    if (!cat) return;
    const suffix = viewDisabledOnly ? "（历史 · 已禁用）" : "（历史）";
    setRulesModal({
      category: cat,
      protect: sensitivityToProtectLevel(cfg.sensitivity),
      title: (MODULE_META[key]?.title ?? key) + suffix,
      key,
      disabled: cfg.disabled_rules ?? [],
    });
  };

  return (
    <>
      <div className="app-crs-version-bar">
        <div className="app-crs-version-row">
          <span>
            <strong>CRS 规则集</strong>
          </span>
          <span className="app-crs-version-actions">
            <NerveButton
              size="sm"
              variant="primary"
              disabled={loadingApply}
              onClick={() => void openApply()}
            >
              选择版本
            </NerveButton>
            <NerveButton size="sm" disabled={loadingHistory} onClick={() => void openHistory()}>
              历史版本
            </NerveButton>
          </span>
        </div>
        <div className="app-crs-version-row">
          <span>
            当前版本：<code>{cur}</code> {lagTag}
          </span>
        </div>
        <div className="app-crs-version-row">
          <span>
            系统最新：<code>{sys}</code>
          </span>
        </div>
      </div>

      <NerveModal
        open={applyOpen}
        title="选择 CRS 版本"
        wide
        onClose={() => setApplyOpen(false)}
        footer={<NerveButton onClick={() => setApplyOpen(false)}>关闭</NerveButton>}
      >
        <div className="app-crs-table-wrap">
          <NerveTable
            className="app-crs-ver-table"
            columns={[
              { key: "id", title: "版本 ID", render: (r) => <code>{r.id}</code> },
              {
                key: "time",
                title: "时间",
                render: (r) => <span className="muted small">{formatCrsTime(r.created_at)}</span>,
              },
              { key: "count", title: "规则数", render: (r) => String(r.rule_count) },
              {
                key: "note",
                title: "说明",
                render: (r) => <span className="muted">{r.note || r.crs_version || "—"}</span>,
              },
              {
                key: "actions",
                title: "操作",
                render: (r) => (
                  <NerveButton size="sm" variant="primary" onClick={() => void applyVersion(r.id)}>
                    应用
                  </NerveButton>
                ),
              },
            ]}
            dataSource={versions}
            rowKey={(r) => r.id}
            emptyText="暂无版本"
          />
        </div>
      </NerveModal>

      <NerveModal
        open={historyOpen}
        title="CRS 历史版本"
        wide
        onClose={() => setHistoryOpen(false)}
        footer={<NerveButton onClick={() => setHistoryOpen(false)}>关闭</NerveButton>}
      >
        <div className="app-crs-table-wrap">
          <NerveTable
            className="app-crs-ver-table"
            columns={[
              {
                key: "id",
                title: "版本",
                render: (r) => (
                  <>
                    {r.is_active ? <NerveTag tone="green">当前</NerveTag> : null}{" "}
                    <code>{r.crs_version_id}</code>
                  </>
                ),
              },
              {
                key: "time",
                title: "应用时间",
                render: (r) => <span className="muted small">{formatCrsTime(r.applied_at)}</span>,
              },
              { key: "count", title: "规则数", render: (r) => String(r.rule_count) },
              {
                key: "note",
                title: "说明",
                render: (r) => <span className="muted">{r.note ?? r.crs_version_label ?? "—"}</span>,
              },
              {
                key: "actions",
                title: "操作",
                render: (r) => (
                  <NerveButton size="sm" onClick={() => setHistConfig(r)}>
                    查看配置
                  </NerveButton>
                ),
              },
            ]}
            dataSource={history}
            rowKey={(r) => r.id}
            emptyText="暂无历史记录 — 首次「选择版本并应用」后会自动保存"
          />
        </div>
      </NerveModal>

      <NerveModal
        open={!!histConfig}
        title={histConfig ? `历史配置 · ${histConfig.crs_version_id}` : ""}
        onClose={() => setHistConfig(null)}
        footer={<NerveButton onClick={() => setHistConfig(null)}>关闭</NerveButton>}
      >
        {histConfig ? (
          <>
            <p className="muted small">
              只读快照 · 应用时间 {histConfig.applied_at}
              {histConfig.note ? ` · ${histConfig.note}` : ""}
            </p>
            <NerveTable<{ key: string; cfg: AppCrsHistoryEntry["modules_snapshot"][string] }>
              columns={[
                {
                  key: "mod",
                  title: "模块",
                  render: (row) => {
                    const meta = MODULE_META[row.key] ?? { title: row.key, desc: "" };
                    return (
                      <>
                        <strong>{meta.title}</strong>
                        <div className="muted small">{row.key}</div>
                      </>
                    );
                  },
                },
                {
                  key: "enabled",
                  title: "开关",
                  render: (row) => (
                    <NerveTag tone={row.cfg.enabled ? "green" : "default"}>
                      {row.cfg.enabled ? "开" : "关"}
                    </NerveTag>
                  ),
                },
                {
                  key: "action",
                  title: "动作/灵敏度",
                  render: (row) => `${row.cfg.action} / ${row.cfg.sensitivity}`,
                },
                {
                  key: "disabled",
                  title: "禁用规则",
                  render: (row) => {
                    const n = row.cfg.disabled_rules?.length ?? 0;
                    if (!n) return "—";
                    return (
                      <NerveButton
                        size="sm"
                        variant="link"
                        onClick={() => openHistRules(histConfig, row.key, true)}
                      >
                        已禁用 {n} 条
                      </NerveButton>
                    );
                  },
                },
                {
                  key: "rules",
                  title: "",
                  render: (row) => {
                    const cat = MODULE_CRS_CATEGORY[row.key];
                    if (!cat) return null;
                    return (
                      <NerveButton
                        size="sm"
                        onClick={() => openHistRules(histConfig, row.key, false)}
                      >
                        查看规则
                      </NerveButton>
                    );
                  },
                },
              ]}
              dataSource={Object.entries(histConfig.modules_snapshot).map(([key, cfg]) => ({
                key,
                cfg,
              }))}
              rowKey={(r) => r.key}
              emptyText="无模块快照"
            />
          </>
        ) : null}
      </NerveModal>

      {rulesModal ? (
        <ModuleRulesModal
          open
          category={rulesModal.category}
          protect={rulesModal.protect}
          moduleTitle={rulesModal.title}
          moduleKey={rulesModal.key}
          moduleEnabled={histConfig?.modules_snapshot[rulesModal.key]?.enabled !== false}
          siteId={siteId}
          editable={false}
          disabledRules={rulesModal.disabled}
          onClose={() => setRulesModal(null)}
        />
      ) : null}
    </>
  );
}

/** 加载并展示 CRS 版本栏（含 status 拉取） */
export function AppCrsVersionBarLoader({
  siteId,
  onApplied,
}: {
  siteId: string;
  onApplied: () => void | Promise<void>;
}) {
  const [status, setStatus] = useState<AppCrsStatus | null>(null);

  useEffect(() => {
    void fetchAppCrsStatus(siteId)
      .then(setStatus)
      .catch(() => setStatus(null));
  }, [siteId]);

  if (!status) return null;

  return (
    <AppCrsVersionBar
      siteId={siteId}
      status={status}
      onStatusChange={setStatus}
      onApplied={onApplied}
    />
  );
}
