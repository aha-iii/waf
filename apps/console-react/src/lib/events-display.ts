import { sanitizeProductText } from "../brand-sanitize";
import type { WafEvent } from "./waf-types";

export function triageVerdictLabel(verdict: string): string {
  const map: Record<string, string> = {
    confirmed_attack: "确认攻击",
    likely_attack: "疑似攻击",
    needs_review: "待复核",
    likely_false_positive: "疑似误报",
  };
  return map[verdict] ?? verdict;
}

export function triageVerdictClass(verdict: string): string {
  const map: Record<string, string> = {
    confirmed_attack: "ev-triage-confirmed",
    likely_attack: "ev-triage-likely",
    needs_review: "ev-triage-review",
    likely_false_positive: "ev-triage-fp",
  };
  return map[verdict] ?? "";
}

export function triageTone(verdict: string): "red" | "orange" | "green" | "default" {
  if (verdict === "confirmed_attack") return "red";
  if (verdict === "likely_attack") return "orange";
  if (verdict === "likely_false_positive") return "green";
  return "default";
}

export function confidenceClass(confidence: number): string {
  if (confidence >= 80) return "ev-confidence ev-confidence-high";
  if (confidence >= 60) return "ev-confidence ev-confidence-mid";
  return "ev-confidence ev-confidence-low";
}

export function inspectSourceLabel(source?: string): string {
  switch (source) {
    case "ebpf":
      return "eBPF";
    case "http-bypass":
      return "Gateway";
    case "waf-edge":
      return "WAF Edge";
    case "agent-local":
      return "Agent";
    default:
      return source?.trim() || "—";
  }
}

export function stageLabel(stage?: string): string {
  switch (stage) {
    case "semantic":
      return "核心";
    case "expert":
      return "专家";
    case "core":
      return "核心";
    case "custom":
      return "自定义";
    case "pre_access":
      return "基础";
    case "ai":
      return "AI";
    default:
      return "";
  }
}

/** 旧事件兜底：从 plugin / category / message 推断防护段与规则集 */
export function inferProtectionOrigin(e: WafEvent): { stage?: string; ruleSet?: string } {
  if (e.protection_stage || e.rule_set) {
    if (e.protection_stage === "semantic") {
      return { stage: "core", ruleSet: e.rule_set || "semantic" };
    }
    return { stage: e.protection_stage, ruleSet: e.rule_set };
  }
  const plugin = e.plugin_name || e.message?.match(/plugin=([^\s]+)/)?.[1];
  const engines = e.message?.match(/engines=([^\s]+)/)?.[1]?.split(",")?.[0];
  if (plugin === "nerve-semantic" || e.message?.includes("semantic_score=")) {
    return { stage: "core", ruleSet: engines || "semantic" };
  }
  if (plugin === "libinjection") return { stage: "core", ruleSet: "注入检测" };
  if (plugin === "expert" || plugin === "coraza" || e.category === "expert" || e.category === "crs") {
    return { stage: "expert", ruleSet: "expert-crs" };
  }
  if (plugin === "custom-rules" || e.category === "custom") {
    return { stage: "custom", ruleSet: "custom" };
  }
  if (plugin === "ai-protection" || e.category === "prompt_injection" || e.category === "ai_artifact") {
    return { stage: "ai", ruleSet: "ai-rules" };
  }
  if (plugin === "bot" || plugin === "cc" || plugin === "geo" || plugin === "behavior") {
    return { stage: "pre_access", ruleSet: plugin };
  }
  if (e.rule_id && !/^\d/.test(e.rule_id)) {
    return { stage: "core", ruleSet: "l7-json" };
  }
  return {};
}

export function strategyDisplay(e: WafEvent): { name: string; id?: string } | null {
  const name = e.strategy_name?.trim();
  if (!name) return null;
  return { name, id: e.strategy_id?.trim() || undefined };
}

export function moduleDisplay(e: WafEvent): { stage: string; ruleSet: string } | null {
  const inferred = inferProtectionOrigin(e);
  const rawStage = e.protection_stage === "semantic" ? "core" : e.protection_stage || inferred.stage;
  const stage = stageLabel(rawStage);
  let ruleSet = sanitizeProductText(e.rule_set || inferred.ruleSet || "").trim();
  if (ruleSet === "libinjection") ruleSet = "注入检测";
  if (!stage && !ruleSet) return null;
  return { stage, ruleSet };
}

export function eventRequestMessage(e: WafEvent): string {
  if (e.category === "l3_policy" && e.message) {
    return `${e.message}${e.dst_ip && !e.message.includes(e.dst_ip) ? ` · ${e.dst_ip}` : ""}`;
  }
  return `${e.method ?? ""} ${e.uri ?? e.message ?? "—"}`.trim() || "—";
}

export function eventSample(e: WafEvent): string {
  const sample = e.payload_sample ?? (e.dst_ip ? `deny_ip=${e.dst_ip}` : "");
  return sample.slice(0, 80);
}

export function resolveModuleKeyForEvent(event: WafEvent): string {
  const cat = event.category.trim();
  if (cat && cat !== "custom" && cat !== "l3_policy") return cat;
  const rid = event.rule_id ?? "";
  if (/^\d/.test(rid)) return "crs";
  return "custom";
}
