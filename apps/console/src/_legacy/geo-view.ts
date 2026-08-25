/** Legacy console module: Geo view. */
import { apiGet, apiPost, apiPut } from "./api.js";
import { esc, pageHeader, switchHtml } from "./components/ui.js";

type GeoConfig = {
  enabled: boolean;
  block_countries: string[];
  allow_countries: string[];
};

const COUNTRY_PRESETS = [
  { code: "CN", name: "中国" },
  { code: "US", name: "美国" },
  { code: "RU", name: "俄罗斯" },
  { code: "KP", name: "朝鲜" },
  { code: "IR", name: "伊朗" },
];

export async function renderGeoView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  const res = await apiGet<{ config: GeoConfig }>("/api/geo/config");
  const cfg = res.config;

  root.innerHTML = `
    ${pageHeader("地域封禁", "")}
    <div class="dash-grid-2">
      <div class="ant-card">
        <div class="ant-card-head">Geo 策略</div>
        <div class="ant-card-body form-stack">
          ${switchHtml("geo-enabled", cfg.enabled, "启用地域封禁")}
          <label class="ant-form-label">封禁国家码（逗号分隔 ISO 3166-1 alpha-2）</label>
          <input class="ant-input ant-input-wide" id="geo-block" value="${esc(cfg.block_countries.join(","))}" placeholder="RU,KP,IR" />
          <label class="ant-form-label">仅允许国家（留空表示不限制）</label>
          <input class="ant-input ant-input-wide" id="geo-allow" value="${esc(cfg.allow_countries.join(","))}" />
          <div class="chip-row">${COUNTRY_PRESETS.map((c) =>
            `<button type="button" class="chip-btn" data-code="${c.code}">${c.name} (${c.code})</button>`).join("")}</div>
          <button type="button" class="ant-btn ant-btn-primary" id="btn-save-geo">保存配置</button>
        </div>
      </div>
      <div class="ant-card">
        <div class="ant-card-head">Geo 查询测试</div>
        <div class="ant-card-body form-stack">
          <input class="ant-input ant-input-wide" id="geo-test-ip" placeholder="8.8.8.8" value="8.8.8.8" />
          <button type="button" class="ant-btn" id="btn-geo-lookup">查询</button>
          <pre class="json-view" id="geo-result">—</pre>
        </div>
      </div>
    </div>`;

  root.querySelectorAll(".chip-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const inp = root.querySelector("#geo-block") as HTMLInputElement;
      const code = (btn as HTMLElement).dataset.code!;
      const parts = inp.value.split(",").map((s) => s.trim()).filter(Boolean);
      if (!parts.includes(code)) parts.push(code);
      inp.value = parts.join(",");
    });
  });

  root.querySelector("#btn-save-geo")?.addEventListener("click", async () => {
    const next: GeoConfig = {
      enabled: (root.querySelector("#geo-enabled") as HTMLInputElement).checked,
      block_countries: (root.querySelector("#geo-block") as HTMLInputElement).value.split(",").map((s) => s.trim()).filter(Boolean),
      allow_countries: (root.querySelector("#geo-allow") as HTMLInputElement).value.split(",").map((s) => s.trim()).filter(Boolean),
    };
    await apiPut("/api/geo/config", next);
    toast("Geo 配置已保存");
  });

  root.querySelector("#btn-geo-lookup")?.addEventListener("click", async () => {
    const ip = (root.querySelector("#geo-test-ip") as HTMLInputElement).value.trim();
    const r = await apiPost<Record<string, unknown>>("/api/geo/lookup", { ip });
    root.querySelector("#geo-result")!.textContent = JSON.stringify(r, null, 2);
  });
}
