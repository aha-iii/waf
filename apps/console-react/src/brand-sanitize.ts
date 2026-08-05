/**
 * 产品展示层：屏蔽第三方实现/品牌名（OWASP、ModSecurity、Coraza、libinjection 等）。
 * 内部 API 字段名可保留；凡写入 UI / 事件摘要 / 规则说明的字符串须经此处理。
 */

const REPLACEMENTS: [RegExp, string][] = [
  [/OWASP\s*ModSecurity\s*CRS/gi, "专家规则集"],
  [/OWASP\s*ModSecurity/gi, "专家规则集"],
  [/ModSecurity\s*CRS/gi, "专家规则集"],
  [/OWASP\s*CRS/gi, "核心规则集"],
  [/OWASP/gi, ""],
  [/ModSecurity/gi, "专家引擎"],
  [/Coraza(\s*CRS)?/gi, "专家防护"],
  [/libinjectionrs/gi, "注入检测"],
  [/libinjection/gi, "注入检测"],
  [/Detected via\s+/gi, ""],
];

export function sanitizeProductText(raw: string | null | undefined): string {
  if (!raw) return "";
  let s = raw;
  for (const [re, to] of REPLACEMENTS) {
    s = s.replace(re, to);
  }
  return s.replace(/\s{2,}/g, " ").trim();
}
