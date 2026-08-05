import { NerveLoginPage } from "@nerve/ui";
import { loginLocal } from "../auth";

type Props = {
  onSuccess: () => void;
};

// Demo 模式预填账号；部署时需保证后端存在同名账号。
// mars-test 通过 NERVE_AUTH_BOOTSTRAP_USER/PASSWORD 自动创建（见 deploy/mars-test/.env.example）。
const DEMO_USERNAME = "lucongming_0108";
const DEMO_PASSWORD = "@Lucongming0108";

function demoPrefill(): { username: string; password: string } | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  if (params.get("demo") !== "1") return null;
  return { username: DEMO_USERNAME, password: DEMO_PASSWORD };
}

export function LoginPage({ onSuccess }: Props) {
  const demo = demoPrefill();

  return (
    <NerveLoginPage
      mark="N"
      productName="Nerve WAF"
      productTagline="Web 应用防火墙管理控制台"
      headline="多层 L7 防护 · 智能威胁检测"
      description="统一接入、策略编排与攻击分析，支持私有化部署与本地账号体系。"
      points={[
        "L7 攻击检测与自定义规则",
        "攻击日志 · 防护策略 · 站点管理",
        "私有化部署 · 数据本地存储",
      ]}
      footer="© Nerve WAF · 企业级 Web 应用防火墙"
      initialUsername={demo?.username ?? ""}
      initialPassword={demo?.password ?? ""}
      onLogin={async (username, password) => {
        await loginLocal(username, password);
        onSuccess();
      }}
    />
  );
}
