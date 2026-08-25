/** modules feature: Core Module Card. */
import {
  NerveButton,
  NerveSelect,
  NerveSwitch,
} from "@nerve/ui";
import {
  MODULE_META,
  PRE_ACCESS_MODULE_PAGES,
  RESPONSE_MODULE_PAGES,
  moduleConfigKind,
  type ModuleCfg,
} from "../../module-meta";
import { MODULE_CRS_CATEGORY } from "../../crs-preset";
import {
  type ModuleConfigScope,
  preAccessCardScopeNote,
  preAccessDetailLinkVisible,
} from "./module-config-scope";

const ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
  { value: "challenge", label: "挑战" },
];

const SENS_OPTS = [
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
];

const PRE_ACCESS_ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
  { value: "challenge", label: "挑战" },
];

const RESPONSE_ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
];

export type CoreModuleCardProps = {
  moduleKey: string;
  cfg: ModuleCfg;
  readonly?: boolean;
  configScope?: ModuleConfigScope;
  onChange: (patch: Partial<ModuleCfg>) => void;
  onOpenRules?: () => void;
  onOpenDetail?: () => void;
};

export function CoreModuleCard({
  moduleKey,
  cfg,
  readonly = false,
  configScope = "app",
  onChange,
  onOpenRules,
  onOpenDetail,
}: CoreModuleCardProps) {
  const meta = MODULE_META[moduleKey] ?? { title: moduleKey, desc: "" };
  const kind = moduleConfigKind(moduleKey);
  const crsCat = MODULE_CRS_CATEGORY[moduleKey];
  const configPage = PRE_ACCESS_MODULE_PAGES[moduleKey] ?? RESPONSE_MODULE_PAGES[moduleKey];

  const actionOpts =
    kind === "response"
      ? RESPONSE_ACTION_OPTS
      : kind === "pre_access"
        ? PRE_ACCESS_ACTION_OPTS
        : ACTION_OPTS;

  return (
    <div
      className={`protection-module-card app-mod-card${cfg.enabled ? "" : " mod-card-disabled"}${readonly ? " is-readonly" : ""}`}
      data-key={moduleKey}
      data-module-kind={kind}
    >
      <div className="protection-module-head">
        <strong>{meta.title}</strong>
        <div className="protection-module-head-actions">
          {crsCat && onOpenRules ? (
            <NerveButton size="sm" variant="link" onClick={onOpenRules}>
              {readonly ? "查看规则" : "规则集"}
            </NerveButton>
          ) : null}
          {onOpenDetail ? (
            <NerveButton size="sm" variant="link" onClick={onOpenDetail}>
              {readonly ? "查看配置" : "详细配置"}
            </NerveButton>
          ) : configPage && preAccessDetailLinkVisible(configScope) ? (
            <NerveButton
              size="sm"
              variant="link"
              onClick={() => {
                location.hash = `#/${configPage}`;
              }}
            >
              详细配置
            </NerveButton>
          ) : null}
          <NerveSwitch
            checked={cfg.enabled}
            disabled={readonly}
            onChange={(e) => onChange({ enabled: e.target.checked })}
          />
        </div>
      </div>
      <p className="muted small">
        {meta.desc}
        {kind === "pre_access" || kind === "response"
          ? preAccessCardScopeNote(configScope, kind)
          : ""}
        {kind === "response" && configScope === "global" ? " · 响应体检测，无单规则裁剪" : ""}
        {cfg.disabled_rules?.length ? ` · 已禁用 ${cfg.disabled_rules.length} 条` : ""}
      </p>
      <div className="protection-module-actions">
        <NerveSelect
          value={cfg.action}
          options={actionOpts}
          disabled={readonly}
          onChange={(e) => onChange({ action: e.target.value })}
        />
        {kind === "attack" ? (
          <NerveSelect
            value={cfg.sensitivity}
            options={SENS_OPTS}
            disabled={readonly}
            onChange={(e) => onChange({ sensitivity: e.target.value })}
          />
        ) : (
          <span className="muted small mod-kind-hint">
            {kind === "pre_access" ? "基础" : "响应"}
          </span>
        )}
      </div>
    </div>
  );
}

type SectionProps = {
  label?: string;
  hint?: string;
  entries: [string, ModuleCfg][];
  readonly?: boolean;
  configScope?: ModuleConfigScope;
  onChange: (key: string, patch: Partial<ModuleCfg>) => void;
  onOpenRules?: (key: string) => void;
  onOpenDetail?: (key: string) => void;
};

export function CoreModuleSection({
  label,
  hint,
  entries,
  readonly,
  configScope = "app",
  onChange,
  onOpenRules,
  onOpenDetail,
}: SectionProps) {
  if (entries.length === 0) return null;
  return (
    <section className="core-mod-section">
      {label ? <p className="pl-section-label">{label}</p> : null}
      {hint ? <p className="muted small core-mod-section-hint">{hint}</p> : null}
      <div className="module-grid">
        {entries.map(([key, cfg]) => (
          <CoreModuleCard
            key={key}
            moduleKey={key}
            cfg={cfg}
            readonly={readonly}
            configScope={configScope}
            onChange={(patch) => onChange(key, patch)}
            onOpenRules={onOpenRules ? () => onOpenRules(key) : undefined}
            onOpenDetail={onOpenDetail ? () => onOpenDetail(key) : undefined}
          />
        ))}
      </div>
    </section>
  );
}

export function CoreModuleSections({
  attack,
  preAccess,
  response,
  readonly,
  configScope = "app",
  onChange,
  onOpenRules,
  onOpenDetail,
}: {
  attack: [string, ModuleCfg][];
  preAccess: [string, ModuleCfg][];
  response: [string, ModuleCfg][];
  readonly?: boolean;
  configScope?: ModuleConfigScope;
  onChange: (key: string, patch: Partial<ModuleCfg>) => void;
  onOpenRules?: (key: string) => void;
  onOpenDetail?: (key: string) => void;
}) {
  return (
    <>
      <CoreModuleSection
        entries={attack}
        readonly={readonly}
        configScope={configScope}
        onChange={onChange}
        onOpenRules={onOpenRules}
        onOpenDetail={onOpenDetail}
      />
      <CoreModuleSection
        entries={preAccess}
        readonly={readonly}
        configScope={configScope}
        onChange={onChange}
        onOpenDetail={onOpenDetail}
      />
      <CoreModuleSection
        entries={response}
        readonly={readonly}
        configScope={configScope}
        onChange={onChange}
        onOpenDetail={onOpenDetail}
      />
    </>
  );
}
