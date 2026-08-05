import { PRE_ACCESS_MODULE_KEYS, RESPONSE_MODULE_KEYS } from "../../module-meta";

const BASIC_DETAIL_KEYS = new Set<string>([
  ...PRE_ACCESS_MODULE_KEYS,
  ...RESPONSE_MODULE_KEYS,
]);

export function basicModuleDetailSupported(moduleKey: string): boolean {
  return BASIC_DETAIL_KEYS.has(moduleKey);
}
