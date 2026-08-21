/** Product display sanitizer: strip third-party engine brand names from UI strings. */
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
