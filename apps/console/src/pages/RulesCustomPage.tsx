/** Console page: signature / custom rule sets. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NERVE_COL_ACTIONS,
  NerveButton,
  NerveCheckbox,
  NerveEmpty,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NervePageHeader,
  NervePagination,
  NerveSectionCard,
  NerveSelect,
  NerveSyncActions,
  NerveTable,
  NerveTableActions,
  NerveTag,
  NerveTextarea,
  defaultPaginationState,
  nerveConfirm,
  nerveResult,
  paginate,
} from "@nerve/ui";
import { apiDelete, apiGet, apiPost, apiPut } from "../api";
import { useToast } from "../hooks/useToast";
import { ActionTag } from "./shared";

type RuleSet = {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
};

type CustomRule = {
  id: string;
  rule_set_id: string;
  name: string;
  phase: string;
  priority: number;
  enabled: boolean;
  match_field: string;
  match_op: string;
  match_pattern: string;
  action: string;
  severity: string;
  category: string;
  rule_type?: string;
  plugin_name?: string;
};

type PluginDescriptor = {
  name: string;
  version: string;
  description: string;
  layer: string;
  type?: string;
  enabled: boolean;
};

type RuleFormState = {
  rule_type: string;
  name: string;
  match_field: string;
  match_op: string;
  match_pattern: string;
  plugin_name: string;
  action: string;
  priority: number;
  enabled: boolean;
};

type SetFormState = {
  name: string;
  description: string;
  enabled: boolean;
};

const FIELD_OPTIONS = [
  { value: "uri", label: "URI" },
  { value: "headers", label: "请求头" },
  { value: "body", label: "请求体" },
  { value: "method", label: "Method" },
  { value: "user_agent", label: "User-Agent" },
  { value: "src_ip", label: "源 IP" },
  { value: "all", label: "全部组合" },
];

const OP_OPTIONS = [
  { value: "regex", label: "正则 (regex)" },
  { value: "contains", label: "包含 (contains)" },
  { value: "eq", label: "等于 (eq)" },
];

const ACTION_OPTIONS = [
  { value: "block", label: "block" },
  { value: "log", label: "log" },
  { value: "challenge", label: "challenge" },
];

function ruleTypeLabel(t?: string): string {
  return t === "plugin" ? "自定义插件" : "签名规则";
}

function setNameById(sets: RuleSet[], id: string): string {
  if (!id) return "未绑定";
  return sets.find((s) => s.id === id)?.name ?? id;
}

function emptyRuleForm(rule?: Partial<CustomRule>): RuleFormState {
  return {
    rule_type: rule?.rule_type ?? "signature",
    name: rule?.name ?? "",
    match_field: rule?.match_field ?? "uri",
    match_op: rule?.match_op ?? "regex",
    match_pattern: rule?.match_pattern ?? "",
    plugin_name: rule?.plugin_name ?? "",
    action: rule?.action ?? "block",
    priority: rule?.priority ?? 100,
    enabled: rule?.enabled !== false,
  };
}

function emptySetForm(set?: Partial<RuleSet>): SetFormState {
  return {
    name: set?.name ?? "",
    description: set?.description ?? "",
    enabled: set?.enabled !== false,
  };
}

function toRulePayload(form: RuleFormState, ruleSetId = ""): Record<string, unknown> {
  if (form.rule_type === "plugin") {
    const plugin = form.plugin_name.trim();
    return {
      rule_set_id: ruleSetId,
      name: form.name.trim() || plugin,
      rule_type: "plugin",
      plugin_name: plugin,
      match_field: "plugin",
      match_op: "plugin",
      match_pattern: plugin,
      action: form.action,
      priority: form.priority || 100,
      enabled: form.enabled,
      phase: "request",
      severity: "medium",
      category: "custom",
    };
  }
  return {
    rule_set_id: ruleSetId,
    name: form.name.trim(),
    rule_type: "signature",
    plugin_name: "",
    match_field: form.match_field,
    match_op: form.match_op,
    match_pattern: form.match_pattern.trim(),
    action: form.action,
    priority: form.priority || 100,
    enabled: form.enabled,
    phase: "request",
    severity: "medium",
    category: "custom",
  };
}

export function RulesCustomPage() {
  const toast = useToast();
  const [sets, setSets] = useState<RuleSet[]>([]);
  const [rules, setRules] = useState<CustomRule[]>([]);
  const [plugins, setPlugins] = useState<PluginDescriptor[]>([]);
  const [loading, setLoading] = useState(true);
  const [rulesPage, setRulesPage] = useState(defaultPaginationState());
  const [setsPage, setSetsPage] = useState(defaultPaginationState());

  const [ruleModal, setRuleModal] = useState<{ editing?: CustomRule } | null>(null);
  const [ruleForm, setRuleForm] = useState<RuleFormState>(emptyRuleForm());
  const [testOut, setTestOut] = useState<string | null>(null);
  const [testOpen, setTestOpen] = useState(false);
  const [testUri, setTestUri] = useState("/scan?tool=sqlmap");
  const [testHeaders, setTestHeaders] = useState("");
  const [savingRule, setSavingRule] = useState(false);

  const [setModal, setSetModal] = useState<{ editing?: RuleSet } | null>(null);
  const [setForm, setSetForm] = useState<SetFormState>(emptySetForm());
  const [savingSet, setSavingSet] = useState(false);

  const [bindModal, setBindModal] = useState<{ set: RuleSet; selected: Set<string> } | null>(null);
  const [savingBind, setSavingBind] = useState(false);

  const loadPlugins = useCallback(async () => {
    try {
      const res = await apiGet<{ plugins: PluginDescriptor[] }>("/api/plugins");
      setPlugins(res.plugins ?? []);
    } catch {
      setPlugins([]);
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await apiGet<{ sets: RuleSet[]; rules: CustomRule[] }>("/api/global/custom-rules");
      setSets(data.sets ?? []);
      setRules(
        (data.rules ?? []).map((r) => ({
          ...r,
          rule_type: r.rule_type ?? "signature",
          rule_set_id: r.rule_set_id ?? "",
        })),
      );
    } catch (e) {
      toast(String(e), false);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    void loadPlugins();
    void refresh();
  }, [loadPlugins, refresh]);

  const luaPlugins = useMemo(
    () =>
      plugins.filter(
        (p) => p.layer.toLowerCase() === "l7" && String(p.type ?? "").toLowerCase() === "lua",
      ),
    [plugins],
  );

  const stats = useMemo(() => {
    const unbound = rules.filter((r) => !r.rule_set_id).length;
    const plugin = rules.filter((r) => r.rule_type === "plugin").length;
    return { total: rules.length, sets: sets.length, unbound, plugin };
  }, [rules, sets]);

  const pageRules = paginate(rules, rulesPage.page, rulesPage.pageSize);
  const pageSets = paginate(sets, setsPage.page, setsPage.pageSize);

  const openRuleEditor = (rule?: CustomRule) => {
    setRuleForm(emptyRuleForm(rule));
    setTestOut(null);
    setRuleModal({ editing: rule });
  };

  const openSetEditor = (set?: RuleSet) => {
    setSetForm(emptySetForm(set));
    setSetModal({ editing: set });
  };

  const openBind = (set: RuleSet) => {
    const selected = new Set(rules.filter((r) => r.rule_set_id === set.id).map((r) => r.id));
    setBindModal({ set, selected });
  };

  const validateRule = (): string | null => {
    if (!ruleForm.name.trim() && ruleForm.rule_type !== "plugin") return "请填写名称";
    if (ruleForm.rule_type === "plugin" && !ruleForm.plugin_name.trim()) return "请选择自定义插件";
    if (ruleForm.rule_type === "signature" && !ruleForm.match_pattern.trim()) return "请填写匹配模式";
    if (ruleForm.rule_type === "plugin" && !ruleForm.name.trim() && !ruleForm.plugin_name.trim()) {
      return "请填写名称";
    }
    return null;
  };

  const saveRule = async () => {
    const err = validateRule();
    if (err) {
      await nerveResult({ type: "info", title: "校验失败", message: err });
      return;
    }
    const editing = ruleModal?.editing;
    const ok = await nerveConfirm({
      title: editing ? "编辑规则" : "新建规则",
      message: `确认${editing ? "保存" : "创建"}规则「${ruleForm.name.trim() || ruleForm.plugin_name}」？`,
      okText: "确认",
    });
    if (!ok) return;
    setSavingRule(true);
    try {
      const body = toRulePayload(ruleForm, editing?.rule_set_id ?? "");
      if (editing) {
        await apiPut(`/api/global/custom-rules/rules/${encodeURIComponent(editing.id)}`, body);
      } else {
        await apiPost("/api/global/custom-rules/rules", body);
      }
      setRuleModal(null);
      await nerveResult({
        type: "success",
        title: editing ? "已保存" : "已创建",
        message: String(body.name),
      });
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "保存失败", message: String(e) });
    } finally {
      setSavingRule(false);
    }
  };

  const runTest = async () => {
    if (ruleForm.rule_type === "plugin") {
      toast("插件型规则请部署后在 Gateway 环境测试", false);
      return;
    }
    setTestOpen(true);
  };

  const submitTest = async () => {
    try {
      const body = toRulePayload(ruleForm);
      const res = await apiPost<{ hit: unknown }>("/api/global/custom-rules/test", {
        ...body,
        uri: testUri,
        headers: testHeaders,
        body: "",
      });
      setTestOut(JSON.stringify(res.hit, null, 2));
      setTestOpen(false);
    } catch (e) {
      toast(String(e), false);
    }
  };

  const deleteRule = async (r: CustomRule) => {
    const ok = await nerveConfirm({
      title: "删除规则",
      message: `确定删除规则 ${r.id} 吗？`,
      danger: true,
      okText: "删除",
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/global/custom-rules/rules/${encodeURIComponent(r.id)}`);
      await nerveResult({ type: "success", title: "已删除", message: `规则 ${r.id} 已删除` });
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "删除失败", message: String(e) });
    }
  };

  const saveSet = async () => {
    if (!setForm.name.trim()) {
      await nerveResult({ type: "info", title: "校验失败", message: "请填写名称" });
      return;
    }
    const editing = setModal?.editing;
    const ok = await nerveConfirm({
      title: editing ? "编辑规则集" : "新建规则集",
      message: `确认${editing ? "保存" : "创建"}规则集「${setForm.name.trim()}」？`,
      okText: "确认",
    });
    if (!ok) return;
    setSavingSet(true);
    try {
      const body = {
        name: setForm.name.trim(),
        description: setForm.description.trim(),
        enabled: setForm.enabled,
      };
      if (editing) {
        await apiPut(`/api/global/custom-rules/sets/${encodeURIComponent(editing.id)}`, body);
      } else {
        await apiPost("/api/global/custom-rules/sets", body);
      }
      setSetModal(null);
      await nerveResult({
        type: "success",
        title: editing ? "已保存" : "已创建",
        message: body.name,
      });
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "保存失败", message: String(e) });
    } finally {
      setSavingSet(false);
    }
  };

  const deleteSet = async (s: RuleSet) => {
    const ok = await nerveConfirm({
      title: "删除规则集",
      message: `确定删除规则集「${s.name}」吗？规则将回到未绑定状态。`,
      danger: true,
      okText: "删除",
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/global/custom-rules/sets/${encodeURIComponent(s.id)}`);
      await nerveResult({ type: "success", title: "已删除", message: `规则集「${s.name}」已删除` });
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "删除失败", message: String(e) });
    }
  };

  const saveBind = async () => {
    if (!bindModal) return;
    const ids = [...bindModal.selected];
    const ok = await nerveConfirm({
      title: "绑定规则",
      message: `确认保存规则集绑定（${ids.length} 条）？`,
      okText: "确认",
    });
    if (!ok) return;
    setSavingBind(true);
    try {
      await apiPut(`/api/global/custom-rules/sets/${encodeURIComponent(bindModal.set.id)}/bind`, {
        rule_ids: ids,
      });
      setBindModal(null);
      await nerveResult({ type: "success", title: "已绑定", message: `${ids.length} 条规则` });
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "绑定失败", message: String(e) });
    } finally {
      setSavingBind(false);
    }
  };

  const handleReload = async () => {
    const ok = await nerveConfirm({
      title: "热加载",
      message: "确定热加载自定义规则吗？",
      okText: "确认",
    });
    if (!ok) return;
    try {
      await apiPost("/api/global/custom-rules/reload", {});
      await loadPlugins();
      await nerveResult({ type: "success", title: "已热加载", message: "自定义规则已热加载" });
      toast("已热加载自定义规则");
      await refresh();
    } catch (e) {
      await nerveResult({ type: "error", title: "热加载失败", message: String(e) });
      toast(String(e), false);
    }
  };

  return (
    <div className="cr-page">
      <NervePageHeader
        title="签名规则"
        actions={
          <NerveSyncActions>
            <NerveButton variant="primary" onClick={() => openRuleEditor()}>
              新建规则
            </NerveButton>
            <NerveButton onClick={() => openSetEditor()}>新建规则集</NerveButton>
            <NerveButton onClick={() => void handleReload()}>热加载</NerveButton>
            <NerveButton onClick={() => void refresh()}>刷新</NerveButton>
          </NerveSyncActions>
        }
      />

      <div className="cr-stats-row">
        <div className="stat-pill stat-blue">
          <div className="stat-pill-val">{stats.total}</div>
          <div className="stat-pill-lbl">规则总数</div>
        </div>
        <div className="stat-pill stat-blue">
          <div className="stat-pill-val">{stats.sets}</div>
          <div className="stat-pill-lbl">规则集</div>
        </div>
        <div className={`stat-pill stat-${stats.unbound ? "orange" : "green"}`}>
          <div className="stat-pill-val">{stats.unbound}</div>
          <div className="stat-pill-lbl">未绑定</div>
        </div>
        <div className="stat-pill stat-blue">
          <div className="stat-pill-val">{stats.plugin}</div>
          <div className="stat-pill-lbl">插件规则</div>
        </div>
      </div>

      <NerveSectionCard title="规则库" subtitle="" count={rules.length}>
        {loading ? <div className="page-loading">加载中…</div> : null}
        {!loading && rules.length === 0 ? <NerveEmpty description="暂无规则" /> : null}
        {!loading && rules.length > 0 ? (
          <>
            <NerveTable<CustomRule>
              className="cr-table"
              rowKey="id"
              dataSource={pageRules}
              columns={[
                {
                  key: "type",
                  title: "类型",
                  render: (r) => (
                    <span
                      className={`cr-type-badge ${r.rule_type === "plugin" ? "cr-type-plugin" : "cr-type-signature"}`}
                    >
                      {ruleTypeLabel(r.rule_type)}
                    </span>
                  ),
                },
                {
                  key: "name",
                  title: "名称",
                  render: (r) => (
                    <div>
                      <div className="cr-rule-name">{r.name}</div>
                      <code className="cr-id">{r.id}</code>
                    </div>
                  ),
                },
                {
                  key: "match",
                  title: "匹配 / 插件",
                  render: (r) =>
                    r.rule_type === "plugin" ? (
                      <code className="cr-code">{r.plugin_name || r.match_pattern}</code>
                    ) : (
                      <div>
                        <span className="cr-match mono small">
                          {r.match_field} · {r.match_op}
                        </span>
                        <div className="cr-pattern mono small">
                          {r.match_pattern.slice(0, 48)}
                          {r.match_pattern.length > 48 ? "…" : ""}
                        </div>
                      </div>
                    ),
                },
                { key: "action", title: "动作", render: (r) => <ActionTag action={r.action} /> },
                {
                  key: "priority",
                  title: "优先级",
                  render: (r) => <span className="cr-priority">{r.priority}</span>,
                },
                {
                  key: "set",
                  title: "所属规则集",
                  render: (r) =>
                    !r.rule_set_id ? (
                      <span className="cr-set-badge cr-set-unbound">未绑定</span>
                    ) : (
                      <span className="cr-set-badge">{setNameById(sets, r.rule_set_id)}</span>
                    ),
                },
                {
                  key: "status",
                  title: "状态",
                  render: (r) => (
                    <NerveTag tone={r.enabled ? "green" : "default"}>
                      {r.enabled ? "启用" : "禁用"}
                    </NerveTag>
                  ),
                },
                {
                  key: "actions",
                  title: "操作",
                  className: NERVE_COL_ACTIONS,
                  render: (r) => (
                    <NerveTableActions
                      actions={[
                        { key: "edit", label: "编辑", onClick: () => openRuleEditor(r) },
                        {
                          key: "del",
                          label: "删除",
                          danger: true,
                          onClick: () => void deleteRule(r),
                        },
                      ]}
                    />
                  ),
                },
              ]}
            />
            <NervePagination total={rules.length} {...rulesPage} onChange={setRulesPage} />
          </>
        ) : null}
      </NerveSectionCard>

      <NerveSectionCard title="规则集" subtitle="" count={sets.length}>
        {loading ? <div className="page-loading">加载中…</div> : null}
        {!loading && sets.length === 0 ? <NerveEmpty description="暂无规则集" /> : null}
        {!loading && sets.length > 0 ? (
          <>
            <NerveTable<RuleSet>
              className="cr-table"
              rowKey="id"
              dataSource={pageSets}
              columns={[
                {
                  key: "name",
                  title: "名称",
                  render: (s) => (
                    <div>
                      <div className="cr-set-title">{s.name}</div>
                      <code className="cr-id">{s.id}</code>
                    </div>
                  ),
                },
                {
                  key: "status",
                  title: "状态",
                  render: (s) => (
                    <NerveTag tone={s.enabled ? "green" : "default"}>
                      {s.enabled ? "启用" : "禁用"}
                    </NerveTag>
                  ),
                },
                {
                  key: "bound",
                  title: "已绑定规则",
                  render: (s) => {
                    const bound = rules.filter((r) => r.rule_set_id === s.id);
                    return bound.length > 0 ? (
                      <div className="cr-bound-chips">
                        {bound.map((r) => (
                          <span key={r.id} className="cr-bound-chip">
                            {ruleTypeLabel(r.rule_type)} {r.name}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <span className="muted small">尚未绑定规则</span>
                    );
                  },
                },
                {
                  key: "desc",
                  title: "说明",
                  render: (s) => <span className="muted small">{s.description || "—"}</span>,
                },
                {
                  key: "actions",
                  title: "操作",
                  className: NERVE_COL_ACTIONS,
                  render: (s) => (
                    <NerveTableActions
                      actions={[
                        {
                          key: "bind",
                          label: "绑定规则",
                          primary: true,
                          onClick: () => openBind(s),
                        },
                        { key: "edit", label: "编辑", onClick: () => openSetEditor(s) },
                        {
                          key: "del",
                          label: "删除",
                          danger: true,
                          onClick: () => void deleteSet(s),
                        },
                      ]}
                    />
                  ),
                },
              ]}
            />
            <NervePagination total={sets.length} {...setsPage} onChange={setSetsPage} />
          </>
        ) : null}
      </NerveSectionCard>

      <NerveModal
        open={!!ruleModal}
        title={ruleModal?.editing ? `编辑规则 ${ruleModal.editing.id}` : "新建规则"}
        onClose={() => setRuleModal(null)}
        wide
        footer={
          <div>
            <NerveButton onClick={() => setRuleModal(null)}>取消</NerveButton>
            <NerveButton onClick={() => void runTest()}>测试匹配</NerveButton>
            <NerveButton variant="primary" disabled={savingRule} onClick={() => void saveRule()}>
              {savingRule ? "保存中…" : "保存"}
            </NerveButton>
          </div>
        }
      >
        <NerveForm>
          <NerveFormItem label="实现方式">
            <NerveSelect
              wide
              value={ruleForm.rule_type}
              options={[
                { value: "signature", label: "签名规则（正则 / 包含 / 等于）" },
                { value: "plugin", label: "Lua 插件" },
              ]}
              onChange={(e) => setRuleForm((f) => ({ ...f, rule_type: e.target.value }))}
            />
          </NerveFormItem>
          <NerveFormItem label="名称">
            <NerveInput
              wide
              value={ruleForm.name}
              placeholder="规则显示名称"
              onChange={(e) => setRuleForm((f) => ({ ...f, name: e.target.value }))}
            />
          </NerveFormItem>
          {ruleForm.rule_type !== "plugin" ? (
            <>
              <NerveFormItem label="匹配字段">
                <NerveSelect
                  wide
                  value={ruleForm.match_field}
                  options={FIELD_OPTIONS}
                  onChange={(e) => setRuleForm((f) => ({ ...f, match_field: e.target.value }))}
                />
              </NerveFormItem>
              <NerveFormItem label="匹配方式">
                <NerveSelect
                  wide
                  value={ruleForm.match_op}
                  options={OP_OPTIONS}
                  onChange={(e) => setRuleForm((f) => ({ ...f, match_op: e.target.value }))}
                />
              </NerveFormItem>
              <NerveFormItem label="匹配模式">
                <NerveInput
                  wide
                  value={ruleForm.match_pattern}
                  placeholder="正则或子串"
                  onChange={(e) => setRuleForm((f) => ({ ...f, match_pattern: e.target.value }))}
                />
              </NerveFormItem>
            </>
          ) : (
            <NerveFormItem label="选择插件">
              <NerveSelect
                wide
                value={ruleForm.plugin_name}
                options={
                  luaPlugins.length
                    ? luaPlugins.map((p) => ({
                        value: p.name,
                        label: `${p.name} — ${p.description || p.version}`,
                      }))
                    : [{ value: "", label: "暂无 L7 插件" }]
                }
                onChange={(e) => setRuleForm((f) => ({ ...f, plugin_name: e.target.value }))}
              />
            </NerveFormItem>
          )}
          <NerveFormItem label="动作">
            <NerveSelect
              wide
              value={ruleForm.action}
              options={ACTION_OPTIONS}
              onChange={(e) => setRuleForm((f) => ({ ...f, action: e.target.value }))}
            />
          </NerveFormItem>
          <NerveFormItem label="优先级">
            <NerveInput
              type="number"
              value={ruleForm.priority}
              onChange={(e) =>
                setRuleForm((f) => ({ ...f, priority: Number(e.target.value) || 100 }))
              }
            />
          </NerveFormItem>
          <NerveCheckbox
            label="启用"
            checked={ruleForm.enabled}
            onChange={(e) => setRuleForm((f) => ({ ...f, enabled: e.target.checked }))}
          />
        </NerveForm>
        {testOut ? (
          <pre className="muted small" style={{ marginTop: 8, maxHeight: 120, overflow: "auto" }}>
            {testOut}
          </pre>
        ) : null}
      </NerveModal>

      <NerveModal
        open={testOpen}
        title="测试匹配"
        onClose={() => setTestOpen(false)}
        footer={
          <div>
            <NerveButton onClick={() => setTestOpen(false)}>取消</NerveButton>
            <NerveButton variant="primary" onClick={() => void submitTest()}>
              测试
            </NerveButton>
          </div>
        }
      >
        <NerveForm>
          <NerveFormItem label="测试 URI">
            <NerveInput wide value={testUri} onChange={(e) => setTestUri(e.target.value)} />
          </NerveFormItem>
          <NerveFormItem label="测试 Headers">
            <NerveTextarea
              wide
              rows={2}
              value={testHeaders}
              placeholder="可选，如 X-Forwarded-For: 1.2.3.4"
              onChange={(e) => setTestHeaders(e.target.value)}
            />
          </NerveFormItem>
        </NerveForm>
      </NerveModal>

      <NerveModal
        open={!!setModal}
        title={setModal?.editing ? "编辑规则集" : "新建规则集"}
        onClose={() => setSetModal(null)}
        footer={
          <div>
            <NerveButton onClick={() => setSetModal(null)}>取消</NerveButton>
            <NerveButton variant="primary" disabled={savingSet} onClick={() => void saveSet()}>
              {savingSet ? "保存中…" : "确定"}
            </NerveButton>
          </div>
        }
      >
        <NerveForm>
          <NerveFormItem label="名称">
            <NerveInput
              wide
              value={setForm.name}
              onChange={(e) => setSetForm((f) => ({ ...f, name: e.target.value }))}
            />
          </NerveFormItem>
          <NerveFormItem label="说明">
            <NerveInput
              wide
              value={setForm.description}
              onChange={(e) => setSetForm((f) => ({ ...f, description: e.target.value }))}
            />
          </NerveFormItem>
          <NerveCheckbox
            label="启用"
            checked={setForm.enabled}
            onChange={(e) => setSetForm((f) => ({ ...f, enabled: e.target.checked }))}
          />
        </NerveForm>
      </NerveModal>

      <NerveModal
        open={!!bindModal}
        title={bindModal ? `绑定规则 — ${bindModal.set.name}` : "绑定规则"}
        onClose={() => setBindModal(null)}
        wide
        footer={
          <div>
            <NerveButton onClick={() => setBindModal(null)}>取消</NerveButton>
            <NerveButton
              variant="primary"
              disabled={savingBind || !rules.length}
              onClick={() => void saveBind()}
            >
              {savingBind ? "保存中…" : "确定"}
            </NerveButton>
          </div>
        }
      >
        {!rules.length ? (
          <NerveEmpty description="暂无规则" />
        ) : (
          <div className="nerve-bind-list cr-bind-list">
            {rules.map((r) => {
              const checked = bindModal?.selected.has(r.id) ?? false;
              return (
                <label
                  key={r.id}
                  className={`nerve-bind-item cr-bind-item${checked ? " is-checked" : ""}`}
                >
                  <NerveCheckbox
                    className="cr-bind-check"
                    checked={checked}
                    onChange={(e) => {
                      setBindModal((prev) => {
                        if (!prev) return prev;
                        const next = new Set(prev.selected);
                        if (e.target.checked) next.add(r.id);
                        else next.delete(r.id);
                        return { ...prev, selected: next };
                      });
                    }}
                  />
                  <div className="cr-bind-item-body">
                    <div className="cr-bind-item-head">
                      <span
                        className={`cr-type-badge ${r.rule_type === "plugin" ? "cr-type-plugin" : "cr-type-signature"}`}
                      >
                        {ruleTypeLabel(r.rule_type)}
                      </span>{" "}
                      <strong>{r.name}</strong>
                    </div>
                    <code className="cr-id">{r.id}</code>
                  </div>
                </label>
              );
            })}
          </div>
        )}
      </NerveModal>
    </div>
  );
}
