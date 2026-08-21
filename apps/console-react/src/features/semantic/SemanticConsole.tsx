/** semantic feature: Semantic Console. */
import { useEffect, useState, type ReactNode } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveSelect,
  NerveSwitch,
  NerveTable,
  NerveTextarea,
} from "@nerve/ui";
import { apiGet, apiPost } from "../../api";
import {
  listToLines,
  linesToList,
  mergeSemanticConfig,
  SEMANTIC_NAV_ITEMS,
  type SemanticConfig,
  type SemanticStats,
  type SemanticTab,
} from "../../lib/semantic";

const DECODE_LABELS: Record<string, string> = {
  url: "URL 解码",
  html_entity: "HTML Entity",
  base64: "Base64",
  hex: "Hex（\\x / 0x）",
  unicode: "Unicode（\\u / %u）",
  chr: "CHR（char/chr）",
};

const SCOPE_LABELS: Record<keyof SemanticConfig["scope"], string> = {
  query: "Query 参数",
  body: "Request Body",
  header: "Header",
  cookie: "Cookie",
  path: "路径段",
};

function moveDecodeOrder(order: string[], index: number, dir: "up" | "down"): string[] {
  const j = dir === "up" ? index - 1 : index + 1;
  if (j < 0 || j >= order.length) return order;
  const next = [...order];
  [next[index], next[j]] = [next[j], next[index]];
  return next;
}

export type SemanticConsoleProps = {
  cfg: SemanticConfig;
  onChange: (cfg: SemanticConfig) => void;
  stats?: SemanticStats | null;
  compact?: boolean;
  siteId?: string;
  readonly?: boolean;
  toolbarLeft?: ReactNode;
  toolbarRight?: ReactNode;
  footer?: ReactNode;
  onRefreshStats?: () => void | Promise<void>;
  onSubmitFeedback?: (payload: {
    event_id?: string;
    verdict: string;
    note?: string;
  }) => void | Promise<void>;
  onFeedbackConfigUpdated?: (cfg: SemanticConfig) => void;
  /** Restrict left-nav items (strategy modal and similar) */
  visibleTabs?: SemanticTab[];
};

export function SemanticConsole({
  cfg,
  onChange,
  stats = null,
  compact = false,
  siteId,
  readonly = false,
  toolbarLeft,
  toolbarRight,
  footer,
  onRefreshStats,
  onSubmitFeedback,
  onFeedbackConfigUpdated,
  visibleTabs,
}: SemanticConsoleProps) {
  const [activeTab, setActiveTab] = useState<SemanticTab>("general");
  const [fbEvent, setFbEvent] = useState("");
  const [fbVerdict, setFbVerdict] = useState("false_positive");
  const [fbNote, setFbNote] = useState("");
  const [fbBusy, setFbBusy] = useState(false);

  const patch = (partial: Partial<SemanticConfig>) => onChange({ ...cfg, ...partial });

  useEffect(() => {
    if (visibleTabs?.length && !visibleTabs.includes(activeTab)) {
      setActiveTab(visibleTabs[0]);
    }
  }, [visibleTabs, activeTab]);

  const submitFeedback = async () => {
    if (readonly) return;
    setFbBusy(true);
    try {
      if (onSubmitFeedback) {
        await onSubmitFeedback({
          event_id: fbEvent || undefined,
          verdict: fbVerdict,
          note: fbNote || undefined,
        });
      } else if (siteId) {
        const resp = await apiPost<{ config_updated?: boolean }>("/api/semantic/feedback", {
          site_id: siteId,
          event_id: fbEvent || undefined,
          verdict: fbVerdict,
          note: fbNote || undefined,
        });
        if (resp.config_updated) {
          const cfgRes = await apiGet<{ config: Partial<SemanticConfig> }>(
            `/api/semantic/config?site_id=${encodeURIComponent(siteId)}`,
          );
          const merged = mergeSemanticConfig(cfgRes.config);
          onFeedbackConfigUpdated?.(merged);
          onChange(merged);
        }
      }
      setFbEvent("");
      setFbNote("");
    } finally {
      setFbBusy(false);
    }
  };

  const renderPanel = () => {
    switch (activeTab) {
      case "general":
        return (
          <NerveForm>
            <NerveFormItem label="语义模式">
              <NerveSelect
                wide
                disabled={readonly}
                value={cfg.mode}
                options={[
                  { value: "strict", label: "严格" },
                  { value: "standard", label: "标准" },
                  { value: "relaxed", label: "宽松" },
                ]}
                onChange={(e) => patch({ mode: e.target.value as SemanticConfig["mode"] })}
              />
            </NerveFormItem>
            <NerveSwitch
              label="SQL 意图检测"
              disabled={readonly}
              checked={cfg.intent_enabled}
              onChange={(e) => patch({ intent_enabled: e.target.checked })}
            />
          </NerveForm>
        );
      case "decode":
        return (
          <NerveForm>
            <div className="route-form-grid-2">
              {(Object.keys(DECODE_LABELS) as Array<keyof Omit<SemanticConfig["decode"], "order" | "max_depth" | "max_expand_bytes">>).map(
                (key) => (
                  <NerveSwitch
                    key={key}
                    label={DECODE_LABELS[key]}
                    disabled={readonly}
                    checked={cfg.decode[key]}
                    onChange={(e) =>
                      onChange({ ...cfg, decode: { ...cfg.decode, [key]: e.target.checked } })
                    }
                  />
                ),
              )}
            </div>
            <NerveFormItem label="解码顺序">
              <div id="decode-order">
                {cfg.decode.order.map((op, i) => (
                  <div className="order-row" key={op} data-op={op}>
                    <span className="muted">{i + 1}.</span> <code>{op}</code>
                    {!readonly ? (
                      <>
                        <NerveButton
                          size="sm"
                          disabled={i === 0}
                          onClick={() =>
                            onChange({
                              ...cfg,
                              decode: {
                                ...cfg.decode,
                                order: moveDecodeOrder(cfg.decode.order, i, "up"),
                              },
                            })
                          }
                        >
                          ↑
                        </NerveButton>
                        <NerveButton
                          size="sm"
                          disabled={i === cfg.decode.order.length - 1}
                          onClick={() =>
                            onChange({
                              ...cfg,
                              decode: {
                                ...cfg.decode,
                                order: moveDecodeOrder(cfg.decode.order, i, "down"),
                              },
                            })
                          }
                        >
                          ↓
                        </NerveButton>
                      </>
                    ) : null}
                  </div>
                ))}
              </div>
            </NerveFormItem>
            <NerveFormItem label="递归最大层数">
              <NerveInput
                wide
                type="number"
                min={1}
                max={32}
                disabled={readonly}
                value={String(cfg.decode.max_depth)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    decode: { ...cfg.decode, max_depth: Number(e.target.value) },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="单参数最大展开字节">
              <NerveInput
                wide
                type="number"
                min={1024}
                step={1024}
                disabled={readonly}
                value={String(cfg.decode.max_expand_bytes)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    decode: { ...cfg.decode, max_expand_bytes: Number(e.target.value) },
                  })
                }
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "scope":
        return (
          <NerveForm>
            <div className="route-form-grid-2">
              {(Object.keys(SCOPE_LABELS) as Array<keyof SemanticConfig["scope"]>).map((key) => (
                <NerveSwitch
                  key={key}
                  label={SCOPE_LABELS[key]}
                  disabled={readonly}
                  checked={cfg.scope[key]}
                  onChange={(e) =>
                    onChange({ ...cfg, scope: { ...cfg.scope, [key]: e.target.checked } })
                  }
                />
              ))}
            </div>
          </NerveForm>
        );
      case "whitelist":
        return (
          <NerveForm>
            <NerveFormItem label="跳过参数名（每行一个）">
              <NerveTextarea
                rows={3}
                disabled={readonly}
                placeholder="csrf_token"
                value={listToLines(cfg.whitelist.param_names)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    whitelist: {
                      ...cfg.whitelist,
                      param_names: linesToList(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="路径前缀（每行一个）">
              <NerveTextarea
                rows={3}
                disabled={readonly}
                placeholder="/static"
                value={listToLines(cfg.whitelist.path_prefixes)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    whitelist: {
                      ...cfg.whitelist,
                      path_prefixes: linesToList(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="Content-Type 例外（body 跳过）">
              <NerveTextarea
                rows={2}
                disabled={readonly}
                placeholder="text/plain"
                value={listToLines(cfg.whitelist.content_types)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    whitelist: {
                      ...cfg.whitelist,
                      content_types: linesToList(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "engines":
        return (
          <NerveForm>
            <NerveSwitch
              label="核心段内优先跑语义增强"
              disabled={readonly}
              checked={cfg.engines.semantic_first}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  engines: { ...cfg.engines, semantic_first: e.target.checked },
                })
              }
            />
            <NerveSwitch
              label="语义解码链内嵌检测"
              disabled={readonly}
              checked={cfg.engines.libinjection}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  engines: { ...cfg.engines, libinjection: e.target.checked },
                })
              }
            />
            <NerveSwitch
              label="SQL 意图慢路径"
              disabled={readonly}
              checked={cfg.engines.sql_intent}
              onChange={(e) => {
                const checked = e.target.checked;
                onChange({
                  ...cfg,
                  intent_enabled: checked,
                  engines: { ...cfg.engines, sql_intent: checked },
                });
              }}
            />
            <NerveSwitch
              label="增强未命中时回退 plain 核心检测"
              disabled={readonly}
              checked={cfg.engines.fallback_plain_libinjection}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  engines: { ...cfg.engines, fallback_plain_libinjection: e.target.checked },
                })
              }
            />
            <NerveFormItem label="流水线顺序（逗号分隔）">
              <NerveInput
                wide
                disabled={readonly}
                value={cfg.engines.pipeline_order.join(",")}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    engines: {
                      ...cfg.engines,
                      pipeline_order: e.target.value
                        .split(",")
                        .map((s) => s.trim())
                        .filter(Boolean),
                    },
                  })
                }
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "performance":
        return (
          <NerveForm>
            <NerveFormItem label="单请求最大切片数">
              <NerveInput
                wide
                type="number"
                min={1}
                max={512}
                disabled={readonly}
                value={String(cfg.performance.max_slices_per_request)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    performance: {
                      ...cfg.performance,
                      max_slices_per_request: Number(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="语义检测超时（毫秒）">
              <NerveInput
                wide
                type="number"
                min={5}
                max={500}
                disabled={readonly}
                value={String(cfg.performance.timeout_ms)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    performance: { ...cfg.performance, timeout_ms: Number(e.target.value) },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="SQL 意图慢路径采样率（0–100%）">
              <NerveInput
                wide
                type="number"
                min={0}
                max={100}
                disabled={readonly}
                value={String(cfg.performance.slow_path_sample_rate)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    performance: {
                      ...cfg.performance,
                      slow_path_sample_rate: Number(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "autotune":
        return (
          <NerveForm>
            <NerveSwitch
              label="启用自动调优"
              disabled={readonly}
              checked={cfg.auto_tune.enabled}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  auto_tune: { ...cfg.auto_tune, enabled: e.target.checked },
                })
              }
            />
            <NerveSwitch
              label="同参数误报达阈值 → 自动加参数白名单"
              disabled={readonly}
              checked={cfg.auto_tune.auto_whitelist_param}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  auto_tune: { ...cfg.auto_tune, auto_whitelist_param: e.target.checked },
                })
              }
            />
            <NerveFormItem label="单参数误报阈值（次）">
              <NerveInput
                wide
                type="number"
                min={1}
                disabled={readonly}
                value={String(cfg.auto_tune.false_positive_threshold)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    auto_tune: {
                      ...cfg.auto_tune,
                      false_positive_threshold: Number(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
            <NerveSwitch
              label="窗口内误报过多 → 自动切 relaxed"
              disabled={readonly}
              checked={cfg.auto_tune.auto_relax_mode}
              onChange={(e) =>
                onChange({
                  ...cfg,
                  auto_tune: { ...cfg.auto_tune, auto_relax_mode: e.target.checked },
                })
              }
            />
            <NerveFormItem label="触发 relaxed 的误报总数">
              <NerveInput
                wide
                type="number"
                min={1}
                disabled={readonly}
                value={String(cfg.auto_tune.relax_after_fp_count)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    auto_tune: {
                      ...cfg.auto_tune,
                      relax_after_fp_count: Number(e.target.value),
                    },
                  })
                }
              />
            </NerveFormItem>
            <NerveFormItem label="统计窗口（小时）">
              <NerveInput
                wide
                type="number"
                min={1}
                max={720}
                disabled={readonly}
                value={String(cfg.auto_tune.window_hours)}
                onChange={(e) =>
                  onChange({
                    ...cfg,
                    auto_tune: { ...cfg.auto_tune, window_hours: Number(e.target.value) },
                  })
                }
              />
            </NerveFormItem>
          </NerveForm>
        );
      case "observe":
        if (!stats) {
          return <p className="muted">统计暂不可用</p>;
        }
        return (
          <NerveForm>
            <p>
              检测 <strong>{stats.counters.inspect_total ?? 0}</strong> 次 · 命中{" "}
              <strong>{stats.counters.hits ?? 0}</strong> · 命中率{" "}
              <strong>{stats.hit_rate_pct ?? 0}%</strong> · 白名单跳过{" "}
              {stats.counters.skipped_whitelist ?? 0} · 超时{" "}
              {stats.counters.skipped_timeout ?? 0}
            </p>
            <div className="ant-form-section-title">最近解码链样例</div>
            <NerveTable
              columns={[
                { key: "ts", title: "时间", render: (r) => r.ts },
                {
                  key: "uri",
                  title: "URI",
                  render: (r) => <code>{r.uri.slice(0, 36)}</code>,
                },
                { key: "param_path", title: "参数" },
                { key: "score", title: "分数", render: (r) => String(r.semantic_score) },
                {
                  key: "chain",
                  title: "解码链",
                  render: (r) => <code>{r.decode_chain.join(" → ")}</code>,
                },
              ]}
              dataSource={(stats.recent_samples ?? []).slice(0, 10)}
              rowKey={(_, i) => String(i)}
              emptyText="暂无样例，触发检测后会出现"
            />
            {!readonly ? (
              <>
                <div className="ant-form-section-title">提交反馈</div>
                <NerveFormItem label="事件 ID（可选）">
                  <NerveInput
                    placeholder="事件 ID（可选，自动解析 param/uri）"
                    value={fbEvent}
                    onChange={(e) => setFbEvent(e.target.value)}
                  />
                </NerveFormItem>
                <NerveFormItem label="判定">
                  <NerveSelect
                    wide
                    value={fbVerdict}
                    options={[
                      { value: "false_positive", label: "误报" },
                      { value: "true_positive", label: "确认攻击" },
                      { value: "uncertain", label: "不确定" },
                    ]}
                    onChange={(e) => setFbVerdict(e.target.value)}
                  />
                </NerveFormItem>
                <NerveFormItem label="备注">
                  <NerveInput wide placeholder="备注" value={fbNote} onChange={(e) => setFbNote(e.target.value)} />
                </NerveFormItem>
                <NerveButton disabled={fbBusy} onClick={() => void submitFeedback()}>
                  提交反馈
                </NerveButton>
              </>
            ) : null}
            {(stats.feedback ?? []).slice(0, 8).length > 0 ? (
              <ul className="muted small" style={{ marginTop: 12 }}>
                {(stats.feedback ?? []).slice(0, 8).map((f, i) => (
                  <li key={`${f.ts}-${i}`}>
                    {f.ts} · {f.verdict}
                    {f.applied_action ? (
                      <>
                        {" "}
                        · <strong>{f.applied_action}</strong>
                      </>
                    ) : null}
                    {f.note ? ` — ${f.note}` : ""}
                  </li>
                ))}
              </ul>
            ) : null}
          </NerveForm>
        );
      default:
        return null;
    }
  };

  const navItems = visibleTabs
    ? SEMANTIC_NAV_ITEMS.filter((item) => visibleTabs.includes(item.id))
    : SEMANTIC_NAV_ITEMS;

  const activeLabel = navItems.find((t) => t.id === activeTab)?.label ?? "";

  return (
    <div
      className={`ant-card alert-console${compact ? " alert-console-compact" : ""}`}
      id="app-sem-console"
    >
      <div className="alert-console-toolbar">
        {toolbarLeft}
        <NerveSwitch
          label="启用 NSP"
          disabled={readonly}
          checked={cfg.enabled}
          onChange={(e) => patch({ enabled: e.target.checked })}
        />
        <span className="alert-toolbar-spacer" />
        {onRefreshStats && !readonly ? (
          <NerveButton
            size="sm"
            onClick={() => {
              void onRefreshStats();
              setActiveTab("observe");
            }}
          >
            刷新观测
          </NerveButton>
        ) : null}
        {toolbarRight}
      </div>
      <div className="alert-console-body">
        <nav className="alert-console-nav" id="sem-nav">
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`alert-nav-item${activeTab === item.id ? " active" : ""}`}
              data-sem-tab={item.id}
              onClick={() => setActiveTab(item.id)}
            >
              <span className="alert-nav-icon">{item.icon}</span>
              <span className="alert-nav-text">
                <strong>{item.label}</strong>
                <span className="muted small">{item.desc}</span>
              </span>
            </button>
          ))}
        </nav>
        <div className="alert-console-panel" id="sem-panel">
          <NerveCard title={activeLabel}>{renderPanel()}</NerveCard>
        </div>
      </div>
      {footer ? <div className="alert-console-footer muted small">{footer}</div> : null}
    </div>
  );
}
