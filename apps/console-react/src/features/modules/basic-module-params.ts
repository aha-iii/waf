import { apiGet } from "../../api";
import type { ModuleCfg } from "../../module-meta";

export type CcModuleParams = {
  requests_per_minute: number;
  burst: number;
  block_duration_sec: number;
};

export type BotModuleParams = {
  score_threshold: number;
};

export const DEFAULT_CC_PARAMS: CcModuleParams = {
  requests_per_minute: 120,
  burst: 30,
  block_duration_sec: 300,
};

export const DEFAULT_BOT_PARAMS: BotModuleParams = {
  score_threshold: 50,
};

type ParamCarrier = Partial<ModuleCfg> & Record<string, unknown>;

function numField(obj: ParamCarrier, key: string): number | undefined {
  const v = obj[key];
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string" && v.trim() !== "" && Number.isFinite(Number(v))) {
    return Number(v);
  }
  return undefined;
}

export function pickCcParams(cfg: ParamCarrier): Partial<CcModuleParams> {
  const out: Partial<CcModuleParams> = {};
  const rpm = numField(cfg, "requests_per_minute");
  const burst = numField(cfg, "burst");
  const block = numField(cfg, "block_duration_sec");
  if (rpm !== undefined) out.requests_per_minute = rpm;
  if (burst !== undefined) out.burst = burst;
  if (block !== undefined) out.block_duration_sec = block;
  return out;
}

export function pickBotParams(cfg: ParamCarrier): Partial<BotModuleParams> {
  const t = numField(cfg, "score_threshold");
  return t !== undefined ? { score_threshold: t } : {};
}

export function mergeCcParams(
  cfg: ParamCarrier,
  global: CcModuleParams,
): CcModuleParams {
  const local = pickCcParams(cfg);
  return {
    requests_per_minute: local.requests_per_minute ?? global.requests_per_minute,
    burst: local.burst ?? global.burst,
    block_duration_sec: local.block_duration_sec ?? global.block_duration_sec,
  };
}

/** 策略 / 应用快照展示：仅读本地已存字段，不拉取实时全局 */
export function resolveCcParams(cfg: ParamCarrier): CcModuleParams {
  const local = pickCcParams(cfg);
  return {
    requests_per_minute: local.requests_per_minute ?? DEFAULT_CC_PARAMS.requests_per_minute,
    burst: local.burst ?? DEFAULT_CC_PARAMS.burst,
    block_duration_sec: local.block_duration_sec ?? DEFAULT_CC_PARAMS.block_duration_sec,
  };
}

export function mergeBotParams(
  cfg: ParamCarrier,
  global: BotModuleParams,
): BotModuleParams {
  const local = pickBotParams(cfg);
  return {
    score_threshold: local.score_threshold ?? global.score_threshold,
  };
}

/** 策略 / 应用快照展示：仅读本地已存字段，不拉取实时全局 */
export function resolveBotParams(cfg: ParamCarrier): BotModuleParams {
  const local = pickBotParams(cfg);
  return {
    score_threshold: local.score_threshold ?? DEFAULT_BOT_PARAMS.score_threshold,
  };
}

/** 创建策略 / 套用快照时：把当前全局 CC/Bot 参数写入 modules，形成独立副本 */
export function materializeBasicModuleParams(
  modules: Record<string, ModuleCfg>,
  cc: CcModuleParams,
  bot: BotModuleParams,
): Record<string, ModuleCfg> {
  const next = { ...modules };
  const ccMod = { ...(next.cc ?? { enabled: false, action: "block", sensitivity: "medium" }) };
  next.cc = { ...ccMod, ...ccParamsPatch(mergeCcParams(ccMod, cc)) };
  const botMod = { ...(next.bot ?? { enabled: false, action: "block", sensitivity: "medium" }) };
  next.bot = { ...botMod, ...botParamsPatch(mergeBotParams(botMod, bot)) };
  return next;
}

export async function fetchGlobalCcParams(): Promise<CcModuleParams> {
  try {
    const res = await apiGet<{ config: CcModuleParams }>("/api/cc/config");
    return {
      requests_per_minute: res.config.requests_per_minute ?? DEFAULT_CC_PARAMS.requests_per_minute,
      burst: res.config.burst ?? DEFAULT_CC_PARAMS.burst,
      block_duration_sec:
        res.config.block_duration_sec ?? DEFAULT_CC_PARAMS.block_duration_sec,
    };
  } catch {
    return { ...DEFAULT_CC_PARAMS };
  }
}

export async function fetchGlobalBotParams(): Promise<BotModuleParams> {
  try {
    const res = await apiGet<{ config: BotModuleParams }>("/api/bot/config");
    return {
      score_threshold: res.config.score_threshold ?? DEFAULT_BOT_PARAMS.score_threshold,
    };
  } catch {
    return { ...DEFAULT_BOT_PARAMS };
  }
}

export function ccParamsPatch(params: CcModuleParams): Partial<ModuleCfg> {
  return {
    requests_per_minute: params.requests_per_minute,
    burst: params.burst,
    block_duration_sec: params.block_duration_sec,
  };
}

export function botParamsPatch(params: BotModuleParams): Partial<ModuleCfg> {
  return { score_threshold: params.score_threshold };
}

export function copyBasicModuleFields(
  src: ParamCarrier,
): Partial<ModuleCfg> {
  return {
    ...pickCcParams(src),
    ...pickBotParams(src),
  };
}
