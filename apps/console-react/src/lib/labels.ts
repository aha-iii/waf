export const CATEGORY_LABEL: Record<string, string> = {
  sqli: "SQL 注入",
  xss: "XSS",
  rce: "命令注入",
  path_traversal: "路径穿越",
  ssrf: "SSRF",
  webshell: "Webshell",
  behavior: "行为异常",
  bot: "Bot",
  cc: "CC 攻击",
  geo: "地域",
  intel: "威胁情报",
  misc: "其他",
  protocol: "协议",
  l3_policy: "L3 策略",
  prompt_injection: "Prompt 注入",
  anomaly: "异常行为",
  orphan_connect: "孤儿连接",
  custom: "自定义规则",
  digest: "摘要",
  posture: "安全巡检",
  scan: "安全巡检",
  expert: "专家规则",
};

export function categoryLabel(c: string): string {
  return CATEGORY_LABEL[c] ?? c;
}
