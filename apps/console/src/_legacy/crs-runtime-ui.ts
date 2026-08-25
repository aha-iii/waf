/** Legacy console module: Crs runtime ui. */
import { apiGet, apiPut } from "./api.js";
import { btn, esc, filterSelect } from "./components/ui.js";
import { sanitizeProductText } from "./brand-sanitize.js";

export type CrsEnginePreference = "auto" | "wasm" | "off" | "coraza";

export type CrsCapability = {
  key: string;
  label: string;
  applies: boolean;
  note: string;
};

export type CrsRuntimeStatus = {
  preference: CrsEnginePreference;
  preference_source: string;
  env_override?: string | null;
  effective_engine: "wasm" | "coraza_http" | "libinjection" | "disabled";
  wasm_available: boolean;
  wasm_loaded: boolean;
  wasm_path?: string | null;
  capabilities: CrsCapability[];
};

/** Internal engine value → product-facing copy (pages must not expose implementation names) */
const ENGINE_LABEL: Record<CrsEnginePreference, string> = {
  auto: "自动",
  wasm: "专家防护（默认）",
  off: "关闭专家防护引擎",
  coraza: "专家防护（兼容模式）",
};

const EFFECTIVE_LABEL: Record<CrsRuntimeStatus["effective_engine"], string> = {
  wasm: "专家防护已就绪",
  coraza_http: "专家防护（兼容模式）",
  libinjection: "专家防护引擎未启用",
  disabled: "未启用",
};

export async function fetchCrsRuntime(): Promise<CrsRuntimeStatus> {
  return apiGet<CrsRuntimeStatus>("/api/crs/runtime");
}

export function renderCrsRuntimePanel(status: CrsRuntimeStatus, opts?: { compact?: boolean }): string {
  const compact = opts?.compact === true;
  const engineLabel =
    EFFECTIVE_LABEL[status.effective_engine] ?? String(status.effective_engine ?? "未知");
  const envNote = status.env_override
    ? `<div class="ant-alert ant-alert-warning" style="margin-bottom:12px">环境变量已覆盖控制台配置</div>`
    : "";
  const readyNote = !status.wasm_available
    ? `<div class="ant-alert ant-alert-warning" style="margin-bottom:12px">专家防护资产未就绪</div>`
    : "";

  const capRows = (status.capabilities ?? [])
    .map(
      (c) => `<tr>
        <td>${esc(c.label)}</td>
        <td>${c.applies ? '<span class="ant-tag ant-tag-green">生效</span>' : '<span class="ant-tag ant-tag-default">不生效</span>'}</td>
        <td class="muted small">${esc(productizeCapabilityNote(c.note))}</td>
      </tr>`,
    )
    .join("");

  const engineSelect = status.env_override
    ? `<span class="muted">由运维环境锁定</span>`
    : filterSelect({
        id: "crs-engine-pref",
        label: "专家防护引擎",
        width: 220,
        options: (["auto", "wasm", "off", "coraza"] as CrsEnginePreference[]).map((v) => ({
          value: v,
          label: ENGINE_LABEL[v],
          selected: v === status.preference,
        })),
      });

  const saveBtn = status.env_override
    ? ""
    : btn({ id: "crs-engine-save", label: "保存", variant: "primary", size: "sm" });

  return `
    <div class="card crs-runtime-panel${compact ? " crs-runtime-panel-compact" : ""}" id="crs-runtime-panel">
      <h3>专家防护状态</h3>
      <p class="muted small">当前：<strong>${esc(engineLabel)}</strong>
        · 是否启用请以流水线「专家防护」开关为准
      </p>
      ${envNote}
      ${readyNote}
      <div class="table-toolbar" style="flex-wrap:wrap;gap:12px;margin-bottom:12px">
        ${engineSelect}
        ${saveBtn}
      </div>
      <table class="data-table crs-cap-table">
        <thead><tr><th>配置项</th><th>当前</th><th>说明</th></tr></thead>
        <tbody>${capRows}</tbody>
      </table>
    </div>`;
}

function productizeCapabilityNote(note: string): string {
  return sanitizeProductText(
    note
      .replace(/WASM CRS/gi, "专家防护")
      .replace(/WASM/gi, "专家防护")
      .replace(/Rust L7/gi, "核心防护"),
  );
}

export function bindCrsRuntimePanel(
  root: HTMLElement,
  toast: (m: string, ok?: boolean) => void,
  onSaved?: (status: CrsRuntimeStatus) => void,
): void {
  const saveBtn = root.querySelector<HTMLButtonElement>("#crs-engine-save");
  if (!saveBtn) return;
  saveBtn.addEventListener("click", async () => {
    const sel = root.querySelector<HTMLSelectElement>("#crs-engine-pref");
    const engine = (sel?.value ?? "auto") as CrsEnginePreference;
    saveBtn.disabled = true;
    try {
      const resp = await apiPut<{ ok: boolean; status: CrsRuntimeStatus }>("/api/crs/runtime", {
        engine,
      });
      toast("专家防护设置已保存；边缘进程需重载后完全生效");
      onSaved?.(resp.status);
      const panel = root.querySelector("#crs-runtime-panel");
      const host = panel?.parentElement;
      if (host && resp.status) {
        host.innerHTML = renderCrsRuntimePanel(resp.status);
        bindCrsRuntimePanel(root, toast, onSaved);
      }
    } catch (e) {
      toast(String(e), false);
    } finally {
      saveBtn.disabled = false;
    }
  });
}

export async function mountCrsRuntimeHost(
  root: HTMLElement,
  hostId: string,
  toast: (m: string, ok?: boolean) => void,
  opts?: { compact?: boolean; onSaved?: (status: CrsRuntimeStatus) => void },
): Promise<CrsRuntimeStatus> {
  const status = await fetchCrsRuntime();
  const host = root.querySelector(hostId);
  if (host) {
    host.innerHTML = renderCrsRuntimePanel(status, opts);
    bindCrsRuntimePanel(root, toast, opts?.onSaved);
  }
  return status;
}

/** On the expert whole-pack path, core module toggles do not apply to the expert stage */
export function crsModuleControlsLimited(status: CrsRuntimeStatus): boolean {
  return status.effective_engine === "wasm";
}

export function crsRuleTogglesLimited(status: CrsRuntimeStatus): boolean {
  const cap = status.capabilities.find((c) => c.key === "disabled_rules");
  return cap ? !cap.applies : status.effective_engine === "wasm";
}
