# 同级依赖（为何不能「只 clone 本目录」）

Nerve WAF workspace 通过 **Cargo path** 依赖生态内其它仓。当前解析路径如下（相对本仓库根 `Nerve-waf/Nerve-waf/`）：

| 依赖 | Cargo 坐标 | 期望磁盘路径 | 用途 |
|------|------------|--------------|------|
| Antibody 集成 | `nerve-antibody` | `../Nerve-antibody/crates/integration/nerve-antibody` | inspect `expert` 段 |
| Antibody metrics | `nerve-antibody-metrics` | `../Nerve-antibody/crates/engine/facade/nerve-antibody-metrics` | 可选追踪 |
| Alert client/sdk | `nerve-alert-client` / `nerve-alert-sdk` | `../../Nerve-alert/crates/...` | 告警 dispatch / Console 代理 |

对应 monorepo 布局：

```text
<workspace-root>/                 # 例如 waf/
├── Nerve-alert/                  # 告警（API 完整构建需要）
└── Nerve-waf/
    ├── Nerve-waf/                # 本仓库
    └── Nerve-antibody/           # 专家引擎 + 规则包
```

## 最小可运行组合

| 目标 | 需要 |
|------|------|
| 只编 L7 / core / modules（无 expert、无 alert API） | 仍建议保留 Antibody：当前 `nerve-waf-inspect` **硬依赖** `nerve-antibody` path |
| WAF API + Console（完整） | Nerve-waf + Antibody + Alert + PostgreSQL |
| Edge 数据面 | Nerve-waf（wasm CRS）+ 配置；expert 需 Antibody pack |

> Path 依赖即使写成 optional，Cargo 也会在解析时要求目录存在。真正「单仓 crates.io 发布」需要后续把 client/sdk/antibody 发布为版本化 crate 或 git 依赖；在此之前请用同级 clone。

## 检查与拉取

同级仓需自行放好（见上表）。发版脚本见 `scripts/README.md`（`deploy-test` / `deploy-prod`）。

本地开发：

```bash
cargo run -p nerve-waf-api --release
cd apps/console-react && npm install && npm run dev
```
