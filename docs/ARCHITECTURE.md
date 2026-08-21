# Nerve WAF 架构

产品名 **Nerve WAF**（目录 `Nerve-waf/`）。本产品 = **L7 检测** + **控制面** + **WAF-L3 配置（下发 Agent eBPF）** + **边缘数据面**。  
Crate 前缀仍为 `nerve-waf-*`，见 [NAMING.md](./NAMING.md)。同级依赖见 [DEPENDENCIES.md](./DEPENDENCIES.md)。

平台拓扑 / 集群 NetworkPolicy 属 **Nerve-control**（`:8780`），不是本仓长期职责。

## 与生态组件的关系

| 产品 | 位置 | 职责 |
|------|------|------|
| **Nerve WAF** | `Nerve-waf/Nerve-waf/` | L7 inspect、事件、WAF-L3、Console、edge |
| **Nerve Antibody** | `Nerve-waf/Nerve-antibody/` | 专家引擎 + 规则包（inspect `expert`） |
| **Nerve Gateway** | `Nerve-gateway/` | L7 路由 / 反代 / 调 `/api/inspect` |
| **Nerve Agent** | `Nerve/` | eBPF 执行 WAF-L3 |
| **Nerve-alert** | `Nerve-alert/` | 告警中心（API 经 client 上报） |

## Workspace 分层

```text
crates/
  contract/     nerve-waf-sdk, nerve-waf-dataplane
  engine/       core, inspect, l7, wasm, coraza, normalize, semantic, …
  modules/      cc, geo, bot, behavior, whitelist, intel, …
  controlplane/ api, store, auth, rbac, rules-ops, …
  dataplane/    edge, embed
apps/console-react/
plugins/l7/  plugins/l3/
```

## 检测流水线

```text
HTTP 请求
  → WafTransaction
  → InspectPipeline
      pre_access : CC → Geo → Bot / intel / whitelist …
      expert     : Antibody（需同级仓）
      core       : L7 + libinjection + semantic + WASM/CRS
      custom     : 自定义 / api-sec
  → InspectVerdict（allow / log / block / challenge）
  → 可选事件持久化 + Alert dispatch
```

## 端口（默认）

| 服务 | 端口 |
|------|------|
| WAF API | `8781` |
| Console (Vite) | `5174` |
| Nerve-control | `8780` |
| Nerve Agent | `8766` |
| Nerve-alert | `8790` |

配置：`config.yml` + `.env`（见 `.env.example`）。
