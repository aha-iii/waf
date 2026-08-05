/** 检测流水线阶段（与后端 pipeline_order 一致） */

export const PIPELINE_STAGES: { id: string; label: string; desc: string }[] = [
  {
    id: "pre_access",
    label: "基础防护",
    desc: "接入层引擎：CC 限流、Bot 识别与接入前策略，优先过滤高频与自动化流量",
  },
  {
    id: "expert",
    label: "专家防护",
    desc: "专家引擎：规则匹配 + 语义解码 + 自适应评分多层融合，覆盖 OWASP 高危攻击；按强度档整体启用",
  },
  {
    id: "core",
    label: "核心防护",
    desc: "核心规则引擎：按 SQLi/XSS/RCE 等类型独立配置灵敏度与单规则，可叠加语义增强",
  },
  {
    id: "custom",
    label: "自定义防护",
    desc: "扩展引擎：业务签名规则与插件，适配私有协议与定制检测逻辑",
  },
  {
    id: "ai",
    label: "AI防护",
    desc: "Prompt 注入与 AI 资产访问检测；仅观测，不拦截",
  },
];

/** 流水线能力总览（应用配置 · 检测流水线 Tab） */
export const PIPELINE_CAPABILITY_INTRO =
  "每个请求按顺序经过下列检测引擎：前段负责接入与快速过滤，中段深度分析 Web 攻击，后段可扩展业务与 AI 场景规则。可拖动调整顺序，或单独启停某一段。";

export type PipelineEnabledMap = Record<string, boolean>;

export function defaultPipelineEnabled(): PipelineEnabledMap {
  const base = Object.fromEntries(PIPELINE_STAGES.map((s) => [s.id, true]));
  base.ai = false;
  return base;
}

export function normalizePipelineEnabled(raw?: Partial<PipelineEnabledMap> | null): PipelineEnabledMap {
  const base = defaultPipelineEnabled();
  if (!raw) return base;
  const crs = typeof raw.crs === "boolean" ? raw.crs : undefined;
  for (const st of PIPELINE_STAGES) {
    if (typeof raw[st.id] === "boolean") base[st.id] = raw[st.id]!;
    else if ((st.id === "expert" || st.id === "core") && crs !== undefined) base[st.id] = crs;
  }
  if (typeof raw.ai !== "boolean") base.ai = false;
  return base;
}

/** 历史 `crs` → `expert` + `core`；剥离 `semantic`/`plugin`；按规范顺序补齐缺失段 */
export function normalizePipelineOrder(order: string[]): string[] {
  const ids = PIPELINE_STAGES.map((s) => s.id);
  const seen = new Set<string>();
  const result: string[] = [];
  for (const id of order) {
    if (id === "plugin" || id === "semantic") continue;
    if (id === "crs") {
      for (const mapped of ["expert", "core"]) {
        if (!seen.has(mapped)) {
          seen.add(mapped);
          result.push(mapped);
        }
      }
      continue;
    }
    if (ids.includes(id) && !seen.has(id)) {
      seen.add(id);
      result.push(id);
    }
  }
  for (const id of ids) {
    if (seen.has(id)) continue;
    seen.add(id);
    const canonIdx = ids.indexOf(id);
    let insertAt = result.length;
    for (let i = 0; i < result.length; i++) {
      if (ids.indexOf(result[i]) > canonIdx) {
        insertAt = i;
        break;
      }
    }
    result.splice(insertAt, 0, id);
  }
  return result;
}

export function movePipelineStage(order: string[], index: number, dir: -1 | 1): string[] {
  const next = [...normalizePipelineOrder(order)];
  const j = index + dir;
  if (index < 0 || j < 0 || index >= next.length || j >= next.length) return next;
  [next[index], next[j]] = [next[j], next[index]];
  return next;
}
