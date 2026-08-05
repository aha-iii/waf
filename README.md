# Nerve WAF

南北向安全产品（Rust）：**L7 检测**（对标 ModSecurity/Coraza）+ **WAF-L3 防护/配置**（编译 SyncPlan → Agent eBPF）。

| 产品 | 目录 | 职责 |
|------|------|------|
| **Nerve WAF** | `Nerve-waf/` | L7 inspect、事件、**WAF-L3（黑白名单等→eBPF）**、WAF Console |
| **Nerve Gateway** | `Nerve-gateway/` | L7 路由、反代、Gateway 插件链 |
| **平台控制面** | 目标 `Nerve-control/`（现状部分 API 仍混在本仓 `:8780`） | 拓扑/Docker 库存、集群 NP、平台 RBAC — **非本产品长期职责** |

产品边界：[docs/superpowers/specs/2026-07-16-nerve-product-boundaries-design.md](../docs/superpowers/specs/2026-07-16-nerve-product-boundaries-design.md) · 架构附录：[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)。

## Workspace 核心 Crate

| Crate | 职责 |
|-------|------|
| `nerve-waf-sdk` | 契约类型、WafPlugin trait |
| `nerve-waf-core` | Transaction、InspectPhase、Verdict |
| `nerve-waf-l7` | OWASP 正则检测引擎 |
| `nerve-waf-rules-ops` | CRS 规则包加载与 merge |
| `nerve-waf-plugin-host` | JSON/Lua L7 插件宿主 |
| `nerve-waf-inspect` | 统一检测流水线（CC→Geo→Bot→L7→Lua） |
| `nerve-waf-engine` | **WAF-L3** 编译 → SyncPlan → Agent eBPF（保留在本仓） |
| `nerve-waf-store` | 事件 + WAF 配置（平台库存表将迁出 control-plane） |
| `nerve-waf-api` | WAF API（**:8781**；拓扑/NP 已迁 Control `:8780`） |

扩展：`nerve-waf-{cc,geo,bot,intel,alert,...}`

## 快速开始

> 节点配置：编辑根目录 **`config.yml`**（端口、数据库、Nerve Agent 地址等）  
> 启动说明见 [docs/START-WAF-GATEWAY.md](./docs/START-WAF-GATEWAY.md)

```bash
# WAF API（:8781，L7 + WAF-L3；管理面鉴权默认开）
./scripts/start-waf.sh

# WAF Console (:5174) — 含 L3 防护配置页
./scripts/start-console.sh

# Gateway（独立产品）
cd ../Nerve-gateway && ./scripts/build-all.sh && ./scripts/start-admin.sh
```

```bash
cd Nerve-waf && cargo build --release
```

## 检测流水线

`POST /api/inspect` 走完整流水线（**Gateway / APISIX 旁路入口，保留**）：

```
PreAccess (CC / Geo / Bot) → L7 规则 (embedded + CRS + JSON) → Lua 插件
```

### Inspect 双路径（并存）

| 入口 | 调用方 | 说明 |
|------|--------|------|
| `POST :8781/api/inspect` | **Nerve Gateway**、**APISIX 插件**、Console AI Lab | HTTP 旁路，带鉴权/追踪，事件持久化 |
| Unix Socket → `nerve-waf-agent-inspect` | Nerve Agent eBPF L7 | 同引擎低延迟路径，不经 HTTP |
| `POST :8766/api/waf/inspect` | Agent 本地调试 | 转发至 Unix Socket |

Gateway 配置 `waf_api: http://127.0.0.1:8781`；边缘推荐 `mode=edge`（`fail_open` 默认 false）。

### Coraza + CRS 全集（单进程 WASM，推荐 edge）

Gateway `mode=edge` 在**同一进程**内加载 `coraza-proxy-wasm.wasm`（Proxy-Wasm ABI + wasmtime），**无需** Go `coraza-engine` 伴生进程：

```bash
bash scripts/download-coraza-wasm.sh   # 若 wasm/ 下尚无 .wasm
# 默认：wasm 存在即启用 crs 阶段
export NERVE_WAF_CRS_ENGINE=wasm       # 可选，显式指定
```

| 变量 | 说明 |
|------|------|
| `NERVE_WAF_WASM_CRS_PATH` | wasm 文件路径 |
| `NERVE_WAF_WASM_CRS_CONFIG` | Coraza 插件 JSON（默认全集 CRS） |
| `NERVE_WAF_CRS_ENGINE=coraza` | 回退到 Go HTTP 桥（实验/对照，非 edge 推荐） |

详见 `wasm/README.md`、`crates/engine/nerve-waf-wasm/`。

## 主要 API

- `POST /api/inspect` — 完整检测流水线（**Gateway/APISIX 旁路**）
- `GET|PUT /api/modules` — OWASP 模块
- `GET /api/plugins`、`POST /api/plugins/reload`
- `GET|PUT /api/sites` — WAF 防护站点
- `GET /api/events`、`GET /api/stats/overview`
- `GET|PUT /api/alerts/config`、`GET /api/alerts/logs` — 告警配置与审计记录
- `POST /api/compile`、`POST /api/sync` — **WAF-L3** 编译并同步到 Agent eBPF（保留本产品）
- `POST /api/policy/deny|allow` — 运行时/永久封禁（可与 inspect 事件联动）

> 拓扑 / Docker 库存 / 集群 NetworkPolicy → **Nerve-control `:8780`**。生产 Runbook：`docs/runbooks/PRODUCTION.md`。
> Helm：`Nerve-waf/deploy/helm/nerve-waf`。

## 插件目录

```
plugins/l7/    JSON 规则包 + Lua 检测脚本
plugins/l3/    WAF-L3 说明（Rust 内置：黑白名单、端口封禁等）
rules/crs/     CRS 规则包
```

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `PORT` | `8781` | WAF API |
| `NERVE_URL` | `http://127.0.0.1:8766` | Nerve Agent（WAF-L3 / deny 落地） |
| `NERVE_AUTH_ENABLED` | `1`（默认） | 管理面鉴权；`0` 关闭 |
| `NERVE_API_KEY` | — | 鉴权密钥（与 Control/Console 一致） |
| `NERVE_WAF_DATA` | `./data` | 配置目录 |
| `NERVE_WAF_PLUGINS` | `./plugins` | 外置插件 |
| `DATABASE_URL` | — | PostgreSQL（与 Control 共用库 `nerve`） |
