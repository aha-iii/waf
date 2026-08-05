# Nerve WAF 架构

对标 ModSecurity / Coraza / 商业 WAF：用 **Rust workspace 分层**。本产品 = **L7 检测** + **WAF-L3 防护/配置（下发 Agent eBPF）**。

平台拓扑/集群 NP 属 **Nerve-control-plane**（见仓库根  
`docs/superpowers/specs/2026-07-16-nerve-product-boundaries-design.md`），**不是**本仓长期职责。

## 与 Gateway / Agent 的关系

| 产品 | 目录 | 职责 |
|------|------|------|
| **Nerve WAF** | `Nerve-waf/` | L7 inspect、事件、**WAF-L3 → eBPF**、WAF Console |
| **Nerve Gateway** | `Nerve-gateway/` | L7 路由、反代、Gateway 插件链 |
| **Nerve Agent** | `Nerve/` | eBPF **执行** WAF-L3（及来自 control 的集群策略） |

WAF 提供 `/api/inspect`；Gateway 经插件调用。**WAF 不依赖 Gateway**。WAF-L3 **依赖 Agent** 才能落到 eBPF。

## Workspace 分层

```
crates/
  nerve-waf-sdk/           # 契约：Route、Event、Plugin、ProtectionModules
  nerve-waf-core/          # Transaction、InspectPhase、Verdict（Coraza 风格）
  nerve-waf-l7/            # OWASP 正则检测引擎
  nerve-waf-rules-ops/     # CRS 规则包加载、灰度、merge
  nerve-waf-plugin-host/   # JSON/Lua L7 插件扫描与 HostedL7
  nerve-waf-inspect/       # 统一检测流水线
  nerve-waf-engine/        # WAF-L3 编译 → SyncPlan → Nerve Agent eBPF（保留）
  nerve-waf-plugins/       # 内置 WAF-L3 Rust 插件
  nerve-waf-store/         # 事件 + WAF 配置（库存表将迁 control-plane）
  nerve-waf-api/           # WAF API（目标 :8781；现状可仍 :8780）
  nerve-waf-{cc,geo,bot,...}  # 扩展能力 crate（可 feature-gate）
apps/console/              # WAF Console (:5174)
plugins/l7/                # 外置 L7 检测插件（JSON/Lua）
plugins/l3/                # L3 策略说明（Rust 内置）
rules/crs/                 # CRS 规则包目录
```

## 检测流水线

```
HTTP 请求
  → WafTransaction
  → InspectPipeline
      Phase PreAccess   : CC → Geo → Bot
      Phase RequestBody : L7 正则（embedded + CRS + JSON 插件）
      Phase Script      : Lua 插件链
  → InspectVerdict（allow / log / block）
  → WafEvent（可选持久化）
```

## 告警架构（Nerve-alert 独立产品）

告警已从 WAF API 拆分为 **Nerve-alert**（`:8790`），对标云厂商 **消息中心 + 云监控**：

| 组件 | 职责 |
|------|------|
| **WAF / Gateway / Agent** | 事件源，经 `nerve-alert-client` 上报 |
| **nerve-alert-api** | 策略、冷却、通道扇出、落库、日报/周报/未 ACK JOB |
| **Nerve-alert Console** | 告警配置与审计（`:5175`） |

```
WAF 检测命中
  → nerve-alert-client POST /api/v1/events/dispatch
  → AlertDispatcher（策略 + 冷却 + 扇出）
  → alert_logs 落库
  → JOB 回调 WAF GET /api/internal/stats 做摘要
```

WAF Console 告警页：开发态 Vite 代理 `/api/alerts` → `:8790`；生产态由 `nerve-waf-api` 反向代理到 `NERVE_ALERT_URL`（默认 `http://127.0.0.1:8790`）。也可直接使用 Nerve-alert Console。

详见 `../Nerve-alert/README.md`。

规则 merge 优先级（后者追加，按 rule id 去重）：

1. Embedded OWASP（`nerve-waf-l7/rules/`）
2. CRS / AI 规则包（`rules/crs`、`rules/ai`）
3. 外置 JSON 插件包（`plugins/l7/`）

## 配置存储

| 数据 | 位置 |
|------|------|
| routes / modules / runtime_denies | `store` → 文件或 PG |
| CC / Geo / Alerts / ProtectedSite | `data/platform.json` |
| 攻击事件 | `store` → 文件或 PG |
| 告警审计记录 | `data/alert_logs.json` |
| 拓扑 / Docker 库存 | **将迁出** → Nerve-control-plane Store（现状可能仍在本 API） |

`ProtectedSite`：WAF 防护站点（Host 绑定），**不是** Gateway 路由。新 API：`GET/PUT /api/sites`。

## WAF API 组织（演进方向）

当前路由仍含 phase 命名与平台库存历史包袱。**留在本仓**的 domain：

```
/api/
├── inspect              # 完整流水线检测
├── modules              # OWASP 模块开关
├── plugins              # 插件目录 + reload
├── events/              # 查询 / ingest / stats
├── sites/               # WAF 防护站点
├── policy/              # WAF-L3：routes / compile / sync / deny|allow
└── extensions/          # cc / geo / bot / intel / ...
```

**迁出到 control-plane：** `/api/topology/*`、集群 `/api/np/*`、CNI/Observer 运维 BFF、平台级 auth（若定为平台级）。

## 插件类型

| 类型 | 位置 | 用途 |
|------|------|------|
| rust | `nerve-waf-plugins` | L3 策略（黑白名单、端口封禁） |
| json | `plugins/l7/*/rules/` | L7 正则规则包 |
| lua | `plugins/l7/*/plugin.lua` | 自定义 inspect 脚本 |

## 后续演进

- 从本仓迁出拓扑 / 集群 NP / 运维 BFF → `Nerve-control-plane`（见产品边界文档）
- WAF API 端口目标 `:8781`；与 control `:8780` 分离
- `nerve-waf-admin` lib：从 api 拆出 domain handlers
- Response phase 检测链接入 Gateway
- 规则 regression 测试框架
- 告警 digest JOB 扩展（自定义模板、Slack 分频道）
