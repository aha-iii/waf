/** modules feature: Basic Module Detail Modal. */
import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NerveSelect,
  NerveSwitch,
  nerveResult,
} from "@nerve/ui";
import { apiPut } from "../../api";
import { MODULE_META, moduleConfigKind, type ModuleCfg } from "../../module-meta";
import type { ModuleConfigScope } from "./module-config-scope";
import {
  botParamsPatch,
  ccParamsPatch,
  fetchGlobalBotParams,
  fetchGlobalCcParams,
  mergeBotParams,
  mergeCcParams,
  resolveBotParams,
  resolveCcParams,
  type BotModuleParams,
  type CcModuleParams,
} from "./basic-module-params";

const PRE_ACCESS_ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
  { value: "challenge", label: "挑战" },
];

const RESPONSE_ACTION_OPTS = [
  { value: "block", label: "拦截" },
  { value: "log", label: "仅记录" },
];

export type BasicModuleDetailModalProps = {
  open: boolean;
  moduleKey: string;
  cfg: ModuleCfg;
  configScope: ModuleConfigScope;
  readonly?: boolean;
  onClose: () => void;
  onChange: (patch: Partial<ModuleCfg>) => void;
};

export function BasicModuleDetailModal({
  open,
  moduleKey,
  cfg,
  configScope,
  readonly = false,
  onClose,
  onChange,
}: BasicModuleDetailModalProps) {
  const meta = MODULE_META[moduleKey] ?? { title: moduleKey, desc: "" };
  const kind = moduleConfigKind(moduleKey);
  const actionOpts = kind === "response" ? RESPONSE_ACTION_OPTS : PRE_ACCESS_ACTION_OPTS;

  const [ccParams, setCcParams] = useState<CcModuleParams | null>(null);
  const [botParams, setBotParams] = useState<BotModuleParams | null>(null);
  const [loadingParams, setLoadingParams] = useState(false);
  const [saving, setSaving] = useState(false);

  const loadParams = useCallback(async () => {
    if (moduleKey !== "cc" && moduleKey !== "bot") return;
    setLoadingParams(true);
    try {
      if (configScope === "global") {
        if (moduleKey === "cc") {
          const global = await fetchGlobalCcParams();
          setCcParams(mergeCcParams(cfg, global));
        } else {
          const global = await fetchGlobalBotParams();
          setBotParams(mergeBotParams(cfg, global));
        }
      } else if (moduleKey === "cc") {
        setCcParams(resolveCcParams(cfg));
      } else {
        setBotParams(resolveBotParams(cfg));
      }
    } finally {
      setLoadingParams(false);
    }
  }, [cfg, configScope, moduleKey]);

  useEffect(() => {
    if (!open) return;
    void loadParams();
  }, [open, loadParams]);

  const updateCc = (patch: Partial<CcModuleParams>) => {
    setCcParams((prev) => {
      if (!prev) return prev;
      const next = { ...prev, ...patch };
      onChange(ccParamsPatch(next));
      return next;
    });
  };

  const updateBot = (patch: Partial<BotModuleParams>) => {
    setBotParams((prev) => {
      if (!prev) return prev;
      const next = { ...prev, ...patch };
      onChange(botParamsPatch(next));
      return next;
    });
  };

  const confirm = async () => {
    if (readonly) {
      onClose();
      return;
    }
    setSaving(true);
    try {
      if (configScope === "global" && moduleKey === "cc" && ccParams) {
        await apiPut("/api/cc/config", { ...ccParams, enabled: cfg.enabled });
      } else if (configScope === "global" && moduleKey === "bot" && botParams) {
        await apiPut("/api/bot/config", {
          enabled: cfg.enabled,
          action: cfg.action,
          score_threshold: botParams.score_threshold,
        });
      }
      onClose();
    } catch (e) {
      await nerveResult({ type: "error", title: "保存失败", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const scopeNote =
    configScope === "global"
      ? "全局默认：新建策略/应用时会拷贝当前值；已创建的策略与应用不会随全局变更而更新。"
      : configScope === "strategy"
        ? "策略快照：与全局隔离，仅在本策略内修改；需点「套用」或应用内同步才会写入站点。"
        : "本应用快照：与全局隔离；切换/同步策略后才会更新，全局变更不会自动反映在此。";

  return (
    <NerveModal
      open={open}
      title={meta.title}
      onClose={onClose}
      footer={
        <>
          <NerveButton onClick={onClose}>取消</NerveButton>
          <NerveButton variant="primary" disabled={readonly || saving} onClick={() => void confirm()}>
            {saving ? "保存中…" : "确定"}
          </NerveButton>
        </>
      }
    >
      <p className="muted small" style={{ marginBottom: 16 }}>
        {scopeNote}
      </p>

      <NerveForm>
        <NerveSwitch
          label="启用模块"
          checked={cfg.enabled}
          disabled={readonly}
          onChange={(e) => onChange({ enabled: e.target.checked })}
        />
        <NerveFormItem label="处置动作">
          <NerveSelect
            value={cfg.action}
            options={actionOpts}
            disabled={readonly}
            style={{ width: 160 }}
            onChange={(e) => onChange({ action: e.target.value })}
          />
        </NerveFormItem>

        {moduleKey === "cc" ? (
          <>
            <p className="pl-section-label" style={{ marginTop: 8 }}>
              限流参数
            </p>
            {loadingParams || !ccParams ? (
              <p className="muted small">加载中…</p>
            ) : (
              <>
                <NerveFormItem label="每分钟请求数">
                  <NerveInput
                    type="number"
                    value={String(ccParams.requests_per_minute)}
                    disabled={readonly}
                    onChange={(e) => updateCc({ requests_per_minute: Number(e.target.value) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="突发容量">
                  <NerveInput
                    type="number"
                    value={String(ccParams.burst)}
                    disabled={readonly}
                    onChange={(e) => updateCc({ burst: Number(e.target.value) })}
                  />
                </NerveFormItem>
                <NerveFormItem label="封禁时长（秒）">
                  <NerveInput
                    type="number"
                    value={String(ccParams.block_duration_sec)}
                    disabled={readonly}
                    onChange={(e) => updateCc({ block_duration_sec: Number(e.target.value) })}
                  />
                </NerveFormItem>
              </>
            )}
          </>
        ) : null}

        {moduleKey === "bot" ? (
          <>
            <p className="pl-section-label" style={{ marginTop: 8 }}>
              识别参数
            </p>
            {loadingParams || !botParams ? (
              <p className="muted small">加载中…</p>
            ) : (
              <NerveFormItem label="Bot 评分阈值">
                <NerveInput
                  type="number"
                  value={String(botParams.score_threshold)}
                  disabled={readonly}
                  onChange={(e) => updateBot({ score_threshold: Number(e.target.value) })}
                />
              </NerveFormItem>
            )}
          </>
        ) : null}
      </NerveForm>
    </NerveModal>
  );
}
