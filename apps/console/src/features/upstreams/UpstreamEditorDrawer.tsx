/** upstreams feature: Upstream Editor Drawer. */
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import {
  NerveButton,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NervePanelCard,
  NerveSelect,
  NerveSwitch,
  NerveTag,
  nerveConfirm,
} from "@nerve/ui";
import { useToast } from "../../hooks/useToast";
import { UpstreamNodeTable } from "./UpstreamNodeTable";
import type {
  ActiveHealthCheck,
  PassiveHealthCheck,
  UpstreamChecks,
  UpstreamOriginKind,
  UpstreamTransport,
  WafUpstream,
} from "./upstream-types";
import {
  defaultActiveCheck,
  defaultPassiveCheck,
  normalizeUpstream,
  parseStatusList,
  statusListToString,
  upstreamOriginKind,
  upstreamOriginKindLabel,
} from "./upstream-types";

type Props = {
  open: boolean;
  upstream: WafUpstream;
  onClose: () => void;
  onSave: (upstream: WafUpstream) => void;
};

function numVal(v: string, fallback: number): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
}

function FieldGrid({ children, className }: { children: React.ReactNode; className?: string }) {
  return <div className={["upstream-field-grid", className].filter(Boolean).join(" ")}>{children}</div>;
}

export function UpstreamEditorDrawer({ open, upstream, onClose, onSave }: Props) {
  const toast = useToast();
  const [draft, setDraft] = useState<WafUpstream>(upstream);

  useEffect(() => {
    if (open) setDraft(normalizeUpstream(upstream));
  }, [open, upstream]);

  if (!open) return null;

  const transport: UpstreamTransport = draft.transport ?? {};
  const checks: UpstreamChecks = draft.checks ?? {};
  const active = checks.active ?? null;
  const passive = checks.passive ?? null;

  const patchChecks = (patch: Partial<UpstreamChecks>) => {
    setDraft((d) => ({ ...d, checks: { ...d.checks, ...patch }, health_check: undefined }));
  };

  const patchActive = (patch: Partial<ActiveHealthCheck>) => {
    patchChecks({ active: { ...(active ?? defaultActiveCheck()), ...patch } });
  };

  const patchPassive = (patch: Partial<PassiveHealthCheck>) => {
    patchChecks({ passive: { ...(passive ?? defaultPassiveCheck()), ...patch } });
  };

  const patchTransport = (patch: Partial<UpstreamTransport>) => {
    setDraft((d) => ({ ...d, transport: { ...transport, ...patch } }));
  };

  const handleSchemeChange = (scheme: string) => {
    setDraft((d) => ({
      ...d,
      scheme,
      nodes: d.nodes.map((n) => {
        if (scheme === "https" && n.port === 80) return { ...n, port: 443 };
        if (scheme === "http" && n.port === 443) return { ...n, port: 80 };
        return n;
      }),
    }));
  };

  const handleOk = async () => {
    const id = draft.id.trim();
    const isDirect = upstreamOriginKind(draft) === "direct";
    if (!id) {
      toast("请填写 ID", false);
      return;
    }
    if (!draft.name.trim()) {
      toast("请填写名称", false);
      return;
    }
    if (isDirect && !draft.nodes.some((n) => n.host.trim())) {
      toast("请至少填写一个目标节点 Host", false);
      return;
    }
    const ok = await nerveConfirm({
      title: "保存 Upstream",
      message: "确定保存该 Upstream 配置吗？",
    });
    if (!ok) return;
    const normalized = normalizeUpstream({
      ...draft,
      id,
      name: draft.name.trim(),
      nodes: draft.nodes.filter((n) => n.host.trim()),
    });
    if (!normalized.checks?.active && !normalized.checks?.passive) {
      delete normalized.checks;
    }
    onSave(normalized);
  };

  const activeHealthy = active?.healthy;
  const activeUnhealthy = active?.unhealthy;
  const passiveHealthy = passive?.healthy;
  const passiveUnhealthy = passive?.unhealthy;
  const isDirect = upstreamOriginKind(draft) === "direct";

  return createPortal(
    <div
      className="app-config-overlay"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="app-config-drawer origin-config-drawer upstream-editor-drawer"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="app-config-head upstream-editor-head">
          <div>
            <h3>{draft.name.trim() || "新建 Upstream"}</h3>
            <div className="upstream-editor-head-meta">
              <NerveTag>{draft.id || "—"}</NerveTag>
              <NerveTag tone={draft.enabled ? "green" : "default"}>
                {draft.enabled ? "已启用" : "已停用"}
              </NerveTag>
            </div>
          </div>
          <div className="upstream-editor-head-actions">
            <label className="upstream-enable-inline">
              <span>启用</span>
              <NerveSwitch
                checked={draft.enabled}
                onChange={(e) => setDraft((d) => ({ ...d, enabled: e.target.checked }))}
              />
            </label>
            <NerveButton variant="link" onClick={onClose} aria-label="关闭">
              ✕
            </NerveButton>
          </div>
        </div>

        <div className="app-config-body upstream-editor-body">
          <NerveForm className="origin-upstream-editor">
            <NervePanelCard title="基本信息">
              <FieldGrid className="upstream-basic-grid">
                <NerveFormItem label="ID" required>
                  <NerveInput
                    wide
                    value={draft.id}
                    placeholder="请输入 ID"
                    onChange={(e) => setDraft((d) => ({ ...d, id: e.target.value }))}
                  />
                </NerveFormItem>
                <NerveFormItem label="名称" required>
                  <NerveInput
                    wide
                    value={draft.name}
                    placeholder="请输入名称"
                    onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                  />
                </NerveFormItem>
                <NerveFormItem label="负载均衡">
                  <NerveSelect
                    wide
                    value={draft.type}
                    onChange={(e) => setDraft((d) => ({ ...d, type: e.target.value }))}
                    options={[{ value: "roundrobin", label: "轮询 (roundrobin)" }]}
                  />
                </NerveFormItem>
                <NerveFormItem label="上游类型">
                  <NerveSelect
                    wide
                    value={upstreamOriginKind(draft)}
                    onChange={(e) =>
                      setDraft((d) => ({
                        ...d,
                        origin_kind: e.target.value as UpstreamOriginKind,
                      }))
                    }
                    options={[
                      { value: "direct", label: upstreamOriginKindLabel("direct") },
                      { value: "gateway", label: upstreamOriginKindLabel("gateway") },
                    ]}
                  />
                </NerveFormItem>
              </FieldGrid>
            </NervePanelCard>

            {isDirect ? (
              <>
            <NervePanelCard title="目标节点">
              <UpstreamNodeTable
                nodes={draft.nodes}
                scheme={draft.scheme}
                onChange={(nodes) => setDraft((d) => ({ ...d, nodes }))}
              />
            </NervePanelCard>

            <NervePanelCard title="回源参数">
              <FieldGrid>
                <NerveFormItem label="Host 请求头">
                  <NerveSelect
                    wide
                    value={draft.host_header !== undefined ? "custom" : "pass"}
                    onChange={(e) => {
                      if (e.target.value === "pass") {
                        setDraft((d) => ({ ...d, host_header: undefined }));
                      } else {
                        setDraft((d) => ({ ...d, host_header: d.host_header ?? "" }));
                      }
                    }}
                    options={[
                      { value: "pass", label: "与客户端请求一致" },
                      { value: "custom", label: "自定义" },
                    ]}
                  />
                </NerveFormItem>
                <NerveFormItem label="协议">
                  <NerveSelect
                    wide
                    value={draft.scheme}
                    onChange={(e) => handleSchemeChange(e.target.value)}
                    options={[
                      { value: "http", label: "HTTP" },
                      { value: "https", label: "HTTPS" },
                    ]}
                  />
                </NerveFormItem>
                {draft.host_header !== undefined ? (
                  <NerveFormItem label="自定义 Host" className="upstream-field-span2">
                    <NerveInput
                      wide
                      value={draft.host_header}
                      onChange={(e) =>
                        setDraft((d) => ({ ...d, host_header: e.target.value || undefined }))
                      }
                    />
                  </NerveFormItem>
                ) : null}
                {draft.scheme === "https" ? (
                  <NerveFormItem label="SNI" className="upstream-field-span2">
                    <NerveInput
                      wide
                      value={draft.sni ?? ""}
                      onChange={(e) =>
                        setDraft((d) => ({ ...d, sni: e.target.value || undefined }))
                      }
                    />
                  </NerveFormItem>
                ) : null}
                <NerveFormItem label="重试次数">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.retry_count ?? 3)}
                    onChange={(e) => patchTransport({ retry_count: numVal(e.target.value, 3) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="重试超时（秒）">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.retry_timeout_sec ?? 60)}
                    onChange={(e) => patchTransport({ retry_timeout_sec: numVal(e.target.value, 60) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="连接超时（秒）">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.connect_timeout_sec ?? 300)}
                    onChange={(e) =>
                      patchTransport({ connect_timeout_sec: numVal(e.target.value, 300) })
                    }
                  />
                </NerveFormItem>
                <NerveFormItem label="发送超时（秒）">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.send_timeout_sec ?? 300)}
                    onChange={(e) => patchTransport({ send_timeout_sec: numVal(e.target.value, 300) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="接收超时（秒）">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.receive_timeout_sec ?? 300)}
                    onChange={(e) =>
                      patchTransport({ receive_timeout_sec: numVal(e.target.value, 300) })
                    }
                  />
                </NerveFormItem>
              </FieldGrid>
            </NervePanelCard>

            <NervePanelCard title="连接池">
              <FieldGrid>
                <NerveFormItem label="容量">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.pool_capacity ?? 320)}
                    onChange={(e) => patchTransport({ pool_capacity: numVal(e.target.value, 320) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="空闲超时（秒）">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.pool_idle_timeout_sec ?? 60)}
                    onChange={(e) =>
                      patchTransport({ pool_idle_timeout_sec: numVal(e.target.value, 60) })
                    }
                  />
                </NerveFormItem>
                <NerveFormItem label="单连接最大请求数">
                  <NerveInput
                    sm
                    type="number"
                    value={String(transport.pool_max_requests ?? 1000)}
                    onChange={(e) =>
                      patchTransport({ pool_max_requests: numVal(e.target.value, 1000) })
                    }
                  />
                </NerveFormItem>
              </FieldGrid>
            </NervePanelCard>

            <NervePanelCard title="健康检查">
              <div className={`upstream-hc-block${active ? " open" : ""}`}>
                <div className="upstream-hc-head">
                  <strong>主动检查</strong>
                  <NerveSwitch
                    checked={Boolean(active)}
                    onChange={(e) => {
                      patchChecks({ active: e.target.checked ? defaultActiveCheck() : null });
                    }}
                  />
                </div>
                {active && activeHealthy && activeUnhealthy ? (
                  <FieldGrid className="upstream-hc-fields">
                    <NerveFormItem label="探测类型">
                      <NerveSelect
                        wide
                        value={active.type}
                        onChange={(e) => patchActive({ type: e.target.value })}
                        options={[
                          { value: "http", label: "HTTP" },
                          { value: "https", label: "HTTPS" },
                          { value: "tcp", label: "TCP" },
                        ]}
                      />
                    </NerveFormItem>
                    <NerveFormItem label="超时（秒）">
                      <NerveInput
                        sm
                        type="number"
                        value={String(active.timeout)}
                        onChange={(e) => patchActive({ timeout: numVal(e.target.value, 1) })}
                      />
                    </NerveFormItem>
                    {active.type !== "tcp" ? (
                      <NerveFormItem label="请求路径" className="upstream-field-span2">
                        <NerveInput
                          wide
                          value={active.http_path}
                          onChange={(e) => patchActive({ http_path: e.target.value })}
                        />
                      </NerveFormItem>
                    ) : null}
                    <NerveFormItem label="健康间隔（秒）">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeHealthy.interval)}
                        onChange={(e) =>
                          patchActive({
                            healthy: { ...activeHealthy, interval: numVal(e.target.value, 10) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="连续成功次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeHealthy.successes)}
                        onChange={(e) =>
                          patchActive({
                            healthy: { ...activeHealthy, successes: numVal(e.target.value, 2) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="健康状态码" className="upstream-field-span2">
                      <NerveInput
                        wide
                        placeholder="200, 302"
                        value={statusListToString(activeHealthy.http_statuses)}
                        onChange={(e) =>
                          patchActive({
                            healthy: {
                              ...activeHealthy,
                              http_statuses: parseStatusList(e.target.value, activeHealthy.http_statuses),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="不健康间隔（秒）">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeUnhealthy.interval)}
                        onChange={(e) =>
                          patchActive({
                            unhealthy: { ...activeUnhealthy, interval: numVal(e.target.value, 10) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="HTTP 失败次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeUnhealthy.http_failures)}
                        onChange={(e) =>
                          patchActive({
                            unhealthy: {
                              ...activeUnhealthy,
                              http_failures: numVal(e.target.value, 5),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="TCP 失败次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeUnhealthy.tcp_failures)}
                        onChange={(e) =>
                          patchActive({
                            unhealthy: {
                              ...activeUnhealthy,
                              tcp_failures: numVal(e.target.value, 2),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="超时次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(activeUnhealthy.timeouts)}
                        onChange={(e) =>
                          patchActive({
                            unhealthy: { ...activeUnhealthy, timeouts: numVal(e.target.value, 3) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="不健康状态码" className="upstream-field-span2">
                      <NerveInput
                        wide
                        placeholder="429, 500, 503"
                        value={statusListToString(activeUnhealthy.http_statuses)}
                        onChange={(e) =>
                          patchActive({
                            unhealthy: {
                              ...activeUnhealthy,
                              http_statuses: parseStatusList(
                                e.target.value,
                                activeUnhealthy.http_statuses,
                              ),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                  </FieldGrid>
                ) : null}
              </div>

              <div className={`upstream-hc-block${passive ? " open" : ""}`}>
                <div className="upstream-hc-head">
                  <strong>被动检查</strong>
                  <NerveSwitch
                    checked={Boolean(passive)}
                    onChange={(e) => {
                      patchChecks({ passive: e.target.checked ? defaultPassiveCheck() : null });
                    }}
                  />
                </div>
                {passive && passiveHealthy && passiveUnhealthy ? (
                  <FieldGrid className="upstream-hc-fields">
                    <NerveFormItem label="连续成功次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(passiveHealthy.successes)}
                        onChange={(e) =>
                          patchPassive({
                            healthy: { ...passiveHealthy, successes: numVal(e.target.value, 5) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="健康状态码" className="upstream-field-span2">
                      <NerveInput
                        wide
                        placeholder="200, 201, 302"
                        value={statusListToString(passiveHealthy.http_statuses)}
                        onChange={(e) =>
                          patchPassive({
                            healthy: {
                              ...passiveHealthy,
                              http_statuses: parseStatusList(
                                e.target.value,
                                passiveHealthy.http_statuses,
                              ),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="HTTP 失败次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(passiveUnhealthy.http_failures)}
                        onChange={(e) =>
                          patchPassive({
                            unhealthy: {
                              ...passiveUnhealthy,
                              http_failures: numVal(e.target.value, 3),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="TCP 失败次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(passiveUnhealthy.tcp_failures)}
                        onChange={(e) =>
                          patchPassive({
                            unhealthy: {
                              ...passiveUnhealthy,
                              tcp_failures: numVal(e.target.value, 2),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="超时次数">
                      <NerveInput
                        sm
                        type="number"
                        value={String(passiveUnhealthy.timeouts)}
                        onChange={(e) =>
                          patchPassive({
                            unhealthy: { ...passiveUnhealthy, timeouts: numVal(e.target.value, 7) },
                          })
                        }
                      />
                    </NerveFormItem>
                    <NerveFormItem label="不健康状态码" className="upstream-field-span2">
                      <NerveInput
                        wide
                        placeholder="429, 500, 503"
                        value={statusListToString(passiveUnhealthy.http_statuses)}
                        onChange={(e) =>
                          patchPassive({
                            unhealthy: {
                              ...passiveUnhealthy,
                              http_statuses: parseStatusList(
                                e.target.value,
                                passiveUnhealthy.http_statuses,
                              ),
                            },
                          })
                        }
                      />
                    </NerveFormItem>
                  </FieldGrid>
                ) : null}
              </div>
            </NervePanelCard>
              </>
            ) : null}
          </NerveForm>
        </div>

        <div className="origin-config-foot">
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" onClick={() => void handleOk()}>
            保存
          </NerveButton>
        </div>
      </div>
    </div>,
    document.body,
  );
}
