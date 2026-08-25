/** Console page: IP allow/deny lists and temporary bans. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePageHeader,
  NervePagination,
  NerveTabBar,
  NerveTable,
  NerveTableActions,
  NERVE_COL_ACTIONS,
  defaultPaginationState,
  paginate,
} from "@nerve/ui";
import {
  apiGet,
  apiListRuntimeDenies,
  apiPost,
  apiTempAllow,
  apiTempDeny,
  type RuntimeDenyEntry,
} from "../api";
import { TempBanDialog } from "../features/access/TempBanDialog";
import { useToast } from "../hooks/useToast";
import { formatDateTimeZh } from "../lib/datetime";
import { defaultExpiresLocal, localDatetimeToIso } from "../lib/temp-deny";

type BlacklistEntry = { ip: string; kind: string; source?: string };
type TabId = "blacklist" | "runtime" | "quick";

export function AccessPage(): JSX.Element {
  const toast = useToast();
  const [tab, setTab] = useState<TabId>("blacklist");
  const [blacklist, setBlacklist] = useState<BlacklistEntry[]>([]);
  const [blackTotal, setBlackTotal] = useState(0);
  const [denies, setDenies] = useState<RuntimeDenyEntry[]>([]);
  const [denyTotal, setDenyTotal] = useState(0);
  const [blPage, setBlPage] = useState(defaultPaginationState());
  const [rtPage, setRtPage] = useState(defaultPaginationState());
  const [blockIp, setBlockIp] = useState("");
  const [blockReason, setBlockReason] = useState("");
  const [blockExpires, setBlockExpires] = useState(() => defaultExpiresLocal(24));
  const [blockNoExpire, setBlockNoExpire] = useState(false);
  const [loading, setLoading] = useState(false);
  const [runtimeBanOpen, setRuntimeBanOpen] = useState(false);

  const loadBlacklist = useCallback(async () => {
    const data = await apiGet<{ total: number; entries: BlacklistEntry[] }>("/api/policy/blacklist");
    setBlacklist(data.entries ?? []);
    setBlackTotal(data.total ?? 0);
  }, []);

  const loadRuntime = useCallback(async () => {
    const data = await apiListRuntimeDenies();
    setDenies(data.denies ?? []);
    setDenyTotal(data.total ?? 0);
  }, []);

  useEffect(() => {
    setLoading(true);
    const fn =
      tab === "blacklist"
        ? loadBlacklist()
        : tab === "runtime"
          ? loadRuntime()
          : Promise.resolve();
    void fn.catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [tab, loadBlacklist, loadRuntime, toast]);

  const handleUnblock = async (ip: string) => {
    try {
      await apiTempAllow(ip);
      toast(`已解封 ${ip}`);
      await loadRuntime();
    } catch (e) {
      toast(String(e), false);
    }
  };

  const handleTempBlock = async () => {
    const ip = blockIp.trim();
    if (!ip) return toast("请输入 IP", false);
    const expires_at = blockNoExpire ? undefined : localDatetimeToIso(blockExpires);
    if (!blockNoExpire && blockExpires.trim() && !expires_at) {
      return toast("到期时间无效", false);
    }
    try {
      await apiTempDeny(ip, {
        source: "console",
        reason: blockReason.trim() || undefined,
        expires_at,
      });
      toast(
        expires_at
          ? `已临时封禁 ${ip}，到期 ${formatDateTimeZh(expires_at)}`
          : `已临时封禁 ${ip}（无到期）`,
      );
      setTab("runtime");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const handlePlaybookBlock = async () => {
    const ip = blockIp.trim();
    const reason = blockReason.trim();
    if (!ip) return toast("请输入 IP", false);
    try {
      await apiPost("/api/playbook/block-ip", { ip, reason: reason || undefined });
      toast(`Playbook 已封禁 ${ip}`);
    } catch (e) {
      toast(String(e), false);
    }
  };

  const blRows = paginate(blacklist, blPage.page, blPage.pageSize);
  const rtRows = paginate(denies, rtPage.page, rtPage.pageSize);

  return (
    <>
      <NervePageHeader title="访问控制" />
      <NerveTabBar
        tabs={[
          { id: "blacklist", label: "IP 黑名单" },
          { id: "runtime", label: "临时封禁" },
          { id: "quick", label: "应急封禁" },
        ]}
        activeId={tab}
        onChange={(id) => setTab(id as TabId)}
      />

      {tab === "blacklist" && (
        <NerveCard title="策略黑名单" tip={`共 ${blackTotal} 条`}>
          {loading ? (
            <p className="muted">加载中…</p>
          ) : (
            <>
              <NerveTable
                columns={[
                  { key: "ip", title: "IP", render: (r) => <code>{r.ip}</code> },
                  { key: "kind", title: "类型" },
                  { key: "source", title: "来源", render: (r) => r.source ?? "—" },
                ]}
                dataSource={blRows}
                rowKey="ip"
                emptyText="暂无永久黑名单"
              />
              <NervePagination
                total={blacklist.length}
                page={blPage.page}
                pageSize={blPage.pageSize}
                onChange={setBlPage}
              />
            </>
          )}
        </NerveCard>
      )}

      {tab === "runtime" && (
        <NerveCard title="运行时临时封禁" tip={`${denyTotal} 条`}>
          <div className="form-actions" style={{ marginBottom: 12 }}>
            <NerveButton size="sm" variant="primary" onClick={() => setRuntimeBanOpen(true)}>
              + 临时封禁
            </NerveButton>
          </div>
          {loading ? (
            <p className="muted">加载中…</p>
          ) : (
            <>
              <NerveTable
                columns={[
                  { key: "ip", title: "IP", render: (r) => <code>{r.ip}</code> },
                  { key: "source", title: "来源" },
                  { key: "reason", title: "原因", render: (r) => r.reason ?? "—" },
                  {
                    key: "expires",
                    title: "过期",
                    render: (r) =>
                      r.expires_at
                        ? formatDateTimeZh(r.expires_at)
                        : "—",
                  },
                  {
                    key: "actions",
                    title: "",
                    className: NERVE_COL_ACTIONS,
                    render: (r) => (
                      <NerveTableActions
                        actions={[
                          {
                            key: "unblock",
                            label: "解封",
                            onClick: () => void handleUnblock(r.ip),
                          },
                        ]}
                      />
                    ),
                  },
                ]}
                dataSource={rtRows}
                rowKey="ip"
                emptyText="暂无临时封禁"
              />
              <NervePagination
                total={denies.length}
                page={rtPage.page}
                pageSize={rtPage.pageSize}
                onChange={setRtPage}
              />
            </>
          )}
        </NerveCard>
      )}

      {tab === "quick" && (
        <NerveCard title="应急封禁 IP">
          <NerveForm>
            <NerveFormItem label="IPv4 地址" required>
              <NerveInput
                wide
                value={blockIp}
                onChange={(e) => setBlockIp(e.target.value)}
                placeholder="请输入 IPv4 地址"
              />
            </NerveFormItem>
            <NerveFormItem label="到期时间" required={!blockNoExpire}>
              <NerveInput
                wide
                type="datetime-local"
                disabled={blockNoExpire}
                value={blockExpires}
                onChange={(e) => setBlockExpires(e.target.value)}
              />
            </NerveFormItem>
            <label
              className="sem-check-row"
              style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 12 }}
            >
              <input
                type="checkbox"
                checked={blockNoExpire}
                onChange={(e) => setBlockNoExpire(e.target.checked)}
              />
              <span>不设到期（需手动解封）</span>
            </label>
            <NerveFormItem label="原因（可选）">
              <NerveInput
                wide
                value={blockReason}
                onChange={(e) => setBlockReason(e.target.value)}
                placeholder="手动封禁"
              />
            </NerveFormItem>
            <div className="form-actions">
              <NerveButton variant="primary" onClick={() => void handleTempBlock()}>
                临时封禁
              </NerveButton>
              <NerveButton onClick={() => void handlePlaybookBlock()}>Playbook 永久封禁</NerveButton>
            </div>
          </NerveForm>
        </NerveCard>
      )}

      <TempBanDialog
        open={runtimeBanOpen}
        source="console"
        onClose={() => setRuntimeBanOpen(false)}
        onDone={() => void loadRuntime()}
      />
    </>
  );
}
