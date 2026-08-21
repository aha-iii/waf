# 命名约定（产品 vs Crate）

| 层 | 名称 | 说明 |
|----|------|------|
| **套件根** | `Nerve-waf/` | 含本产品 + Antibody |
| **本产品目录** | `Nerve-waf/Nerve-waf/` | 控制面 / 检测 / Console / edge |
| **产品名** | Nerve WAF | 文档与对外叙事 |
| **Rust crate** | `nerve-waf-*` | 稳定包名 |
| **二进制** | `nerve-waf-api`、`nerve-waf-edge`、… | 与 crate 一致 |
| **专家引擎** | Nerve Antibody | `Nerve-waf/Nerve-antibody/`；经 `nerve-antibody` 接入 |

历史曾用目录名 `Nerve-defender`；现已统一为 **`Nerve-waf/Nerve-waf`**。crate 前缀不变。
