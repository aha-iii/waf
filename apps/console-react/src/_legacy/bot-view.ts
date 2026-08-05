import { apiPost } from "./api.js";
import { esc, pageHeader } from "./components/ui.js";

const BOT_SAMPLES = [
  { ua: "Mozilla/5.0 (compatible; Googlebot/2.1)", uri: "/robots.txt", label: "Googlebot" },
  { ua: "curl/7.68.0", uri: "/admin", label: "curl 扫描" },
  { ua: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0", uri: "/", label: "正常浏览器" },
  { ua: "python-requests/2.28.0", uri: "/api/login", label: "脚本客户端" },
];

export async function renderBotView(root: HTMLElement, toast: (m: string, ok?: boolean) => void): Promise<void> {
  root.innerHTML = `
    ${pageHeader("Bot 管理", "")}
    <div class="dash-grid-2" style="margin-top:16px">
      <div class="ant-card">
        <div class="ant-card-head">自定义检测</div>
        <div class="ant-card-body form-stack">
          <label class="ant-form-label">User-Agent</label>
          <input class="ant-input ant-input-wide" id="bot-ua" placeholder="Mozilla/5.0 ..." />
          <label class="ant-form-label">URI</label>
          <input class="ant-input ant-input-wide" id="bot-uri" value="/" />
          <button type="button" class="ant-btn ant-btn-primary" id="btn-bot-score">评分</button>
          <div id="bot-result" class="bot-result-box">—</div>
        </div>
      </div>
      <div class="ant-card">
        <div class="ant-card-head">样本测试</div>
        <div class="ant-card-body">
          <div class="sample-list" id="bot-samples"></div>
        </div>
      </div>
    </div>`;

  async function runScore(ua: string, uri: string, outEl: HTMLElement): Promise<void> {
    const r = await apiPost<{ score: number; is_bot: boolean; reason: string }>("/api/bot/score", { user_agent: ua, uri });
    outEl.innerHTML = `
      <div class="bot-score ${r.is_bot ? "bot" : "human"}">
        <div class="bot-score-val">${r.score}</div>
        <div><strong>${r.is_bot ? "Bot / 自动化" : "疑似人类"}</strong><br><span class="muted">${esc(r.reason)}</span></div>
      </div>`;
  }

  root.querySelector("#btn-bot-score")?.addEventListener("click", async () => {
    const ua = (root.querySelector("#bot-ua") as HTMLInputElement).value;
    const uri = (root.querySelector("#bot-uri") as HTMLInputElement).value;
    const out = root.querySelector("#bot-result") as HTMLElement;
    try {
      await runScore(ua, uri, out);
    } catch (e) {
      toast(String(e), false);
    }
  });

  const samplesEl = root.querySelector("#bot-samples")!;
  for (const s of BOT_SAMPLES) {
    const row = document.createElement("div");
    row.className = "sample-row";
    row.innerHTML = `<div><strong>${esc(s.label)}</strong><div class="muted small">${esc(s.ua.slice(0, 50))}…</div></div>
      <button type="button" class="ant-btn ant-btn-sm">测试</button><div class="sample-out"></div>`;
    row.querySelector("button")?.addEventListener("click", async () => {
      const out = row.querySelector(".sample-out") as HTMLElement;
      await runScore(s.ua, s.uri, out);
    });
    samplesEl.appendChild(row);
  }
}
