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

```bash
# 校验相对路径是否存在
bash scripts/check-sibling-deps.sh

# 若你有镜像地址，可设置环境变量后用脚本克隆（见脚本内注释）
# NERVE_ANTIBODY_GIT=... NERVE_ALERT_GIT=... bash scripts/fetch-sibling-deps.sh
```

本地开发也可先：

```bash
bash scripts/start-waf.sh      # API :8781
bash scripts/start-console.sh  # Console（见脚本实际端口）
```
