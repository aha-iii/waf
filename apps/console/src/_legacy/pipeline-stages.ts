/** Legacy selectable pipeline stages (expert → core → custom by default). */
export const PIPELINE_STAGES: { id: string; label: string; desc: string }[] = [
  {
    id: "expert",
    label: "专家防护",
    desc: "Nerve 私有受保护规则集 · 流水线一键启用，不开放模块/单规则裁剪",
  },
  {
    id: "core",
    label: "核心防护",
    desc: "按攻击类型可配置的基础模块（开关与灵敏度）；可叠加语义增强",
  },
  { id: "custom", label: "自定义检测", desc: "签名规则集与自定义插件" },
];

export type PipelineEnabledMap = Record<string, boolean>;

export function defaultPipelineEnabled(): PipelineEnabledMap {
  return Object.fromEntries(PIPELINE_STAGES.map((s) => [s.id, true]));
}

export function normalizePipelineEnabled(raw?: Partial<PipelineEnabledMap> | null): PipelineEnabledMap {
  const base = defaultPipelineEnabled();
  if (!raw) return base;
  const crs = typeof raw.crs === "boolean" ? raw.crs : undefined;
  for (const st of PIPELINE_STAGES) {
    if (typeof raw[st.id] === "boolean") base[st.id] = raw[st.id]!;
    else if ((st.id === "expert" || st.id === "core") && crs !== undefined) base[st.id] = crs;
  }
  return base;
}

/** Legacy `crs` → `expert` + `core`; strip `semantic`/`plugin`; fill the default three stages */
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
    if (!seen.has(id)) result.push(id);
  }
  return result;
}

export function renderPipelineOrderList(
  order: string[],
  enabled: PipelineEnabledMap,
  esc: (s: string) => string,
): string {
  const normalized = normalizePipelineOrder(order);
  const en = normalizePipelineEnabled(enabled);
  return normalized
    .map((id, i) => {
      const st = PIPELINE_STAGES.find((s) => s.id === id)!;
      const on = en[id] !== false;
      return `<div class="pipeline-order-row${on ? "" : " is-disabled"}" draggable="true" data-stage="${esc(id)}" data-i="${i}">
        <span class="pipeline-drag" title="拖动排序">⋮⋮</span>
        <span class="pipeline-num${on ? "" : " is-off"}">${on ? "·" : "—"}</span>
        <div class="pipeline-info">
          <strong>${esc(st.label)}</strong>
          <span class="muted small">${esc(st.desc)}</span>
        </div>
        <label class="pipeline-enable-switch" title="启用此检测段">
          <input type="checkbox" class="pipeline-stage-enabled" ${on ? "checked" : ""} />
          <span>启用</span>
        </label>
        <code class="pipeline-id">${esc(id)}</code>
        <div class="pipeline-move">
          <button type="button" class="ant-btn ant-btn-sm pipeline-up" data-i="${i}" title="上移">↑</button>
          <button type="button" class="ant-btn ant-btn-sm pipeline-down" data-i="${i}" title="下移">↓</button>
        </div>
      </div>`;
    })
    .join("");
}

export function readPipelineOrder(container: HTMLElement | null): string[] {
  if (!container) return [];
  return [...container.querySelectorAll(".pipeline-order-row")].map(
    (r) => (r as HTMLElement).dataset.stage ?? "",
  ).filter(Boolean);
}

export function readPipelineEnabled(container: HTMLElement | null): PipelineEnabledMap {
  const map = defaultPipelineEnabled();
  if (!container) return map;
  container.querySelectorAll<HTMLElement>(".pipeline-order-row").forEach((row) => {
    const id = row.dataset.stage;
    if (!id) return;
    const chk = row.querySelector(".pipeline-stage-enabled") as HTMLInputElement | null;
    map[id] = chk?.checked ?? true;
  });
  return map;
}

export function bindPipelineOrder(container: HTMLElement, onChange?: () => void): void {
  const list = container.querySelector("#app-pipeline-order");
  if (!list) return;

  let dragEl: HTMLElement | null = null;

  list.querySelectorAll<HTMLElement>(".pipeline-order-row").forEach((row) => {
    row.addEventListener("dragstart", (e) => {
      dragEl = row;
      row.classList.add("dragging");
      e.dataTransfer?.setData("text/plain", row.dataset.stage ?? "");
      if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
    });
    row.addEventListener("dragend", () => {
      row.classList.remove("dragging");
      dragEl = null;
      reindexRows(list);
      onChange?.();
    });
    row.addEventListener("dragover", (e) => {
      e.preventDefault();
      if (!dragEl || dragEl === row) return;
      const rect = row.getBoundingClientRect();
      const after = (e.clientY - rect.top) / rect.height > 0.5;
      if (after) row.after(dragEl);
      else row.before(dragEl);
    });
    row.querySelector(".pipeline-stage-enabled")?.addEventListener("change", () => {
      reindexRows(list);
      onChange?.();
    });
  });

  list.querySelectorAll<HTMLButtonElement>(".pipeline-up, .pipeline-down").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const i = Number(btn.dataset.i);
      const rows = [...list.querySelectorAll<HTMLElement>(".pipeline-order-row")];
      const row = rows[i];
      if (!row) return;
      if (btn.classList.contains("pipeline-up") && i > 0) {
        rows[i - 1].before(row);
      } else if (btn.classList.contains("pipeline-down") && i < rows.length - 1) {
        rows[i + 1].after(row);
      }
      reindexRows(list);
      onChange?.();
    });
  });

  reindexRows(list);
}

function reindexRows(list: Element): void {
  const rows = [...list.querySelectorAll<HTMLElement>(".pipeline-order-row")];
  let exec = 0;
  rows.forEach((row, i) => {
    row.dataset.i = String(i);
    const enabled = (row.querySelector(".pipeline-stage-enabled") as HTMLInputElement)?.checked ?? true;
    row.classList.toggle("is-disabled", !enabled);
    const num = row.querySelector(".pipeline-num");
    if (num) {
      if (enabled) {
        exec += 1;
        num.textContent = String(exec);
        num.classList.remove("is-off");
      } else {
        num.textContent = "—";
        num.classList.add("is-off");
      }
    }
    row.querySelectorAll<HTMLElement>(".pipeline-up, .pipeline-down").forEach((b) => {
      b.dataset.i = String(i);
    });
  });
}

export function setPipelineOrder(
  container: HTMLElement,
  order: string[],
  enabled: PipelineEnabledMap,
): void {
  const list = container.querySelector("#app-pipeline-order");
  if (!list) return;
  list.innerHTML = renderPipelineOrderList(order, enabled, (s) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;"),
  );
  bindPipelineOrder(container);
}
