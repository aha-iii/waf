# Nerve WAF Architecture Overview

本文是 **Nerve WAF 单一事实来源（single overview）**：运行时拓扑、状态归属、配置同步、故障域与产品边界。  
代码分层细节见 [ARCHITECTURE.md](./ARCHITECTURE.md)；部署命令见 [installation.md](./installation.md)。

## 1. 产品定位

Nerve WAF 是 **L7 Web 应用防火墙**，由三个可独立扩缩的进程组成：

| 进程 | 角色 | 默认端口 |
|------|------|----------|
| `nerve-waf-controller` | **控制面**：API、策略持久化、快照生成 | 9000 |
| `nerve-waf-agent` | **数据面**：路由/反代 + 检测流水线 | 9080 |
| `nerve-waf-console` | **管理 UI**（可选）：静态资源 + 反代 Controller | 9090 |

**不在本仓库交付范围内**（可选集成，非硬依赖）：见 [concepts/boundaries.md](./concepts/boundaries.md)。

## 2. 运行时拓扑

### 2.1 逻辑架构

```text
                    ┌─────────────────────────────────────┐
                    │           PostgreSQL                 │
                    │  站点 / 模块 / 规则 / 事件 / 用户     │
                    └──────────────────▲──────────────────┘
                                       │ read/write
┌──────────────┐   HTTP :9090    ┌─────┴──────────────┐
│   运维浏览器  │ ──────────────▶│ nerve-waf-console   │
└──────────────┘                 └──────────┬───────────┘
                                            │ proxy
                                            ▼
                                 ┌──────────────────────┐
                                 │ nerve-waf-controller  │  控制面
                                 │  /api/*  策略 CRUD     │
                                 │  /api/waf/edge/* 快照  │
                                 └──────────┬───────────┘
                                            │
                         watch / dataplane │ (HTTP)
                                            ▼
┌──────────────┐   HTTP :9080    ┌──────────────────────┐
│  客户端流量   │ ──────────────▶│ nerve-waf-agent       │  数据面
└──────────────┘                 │  inspect + proxy      │
                                 └──────────┬───────────┘
                                            │
                                            ▼
                                    上游业务服务
```

### 2.2 生产流量路径（参考）

```text
Internet → TLS 终止 (LB/Ingress) → nerve-waf-agent:9080 → upstream
                管理路径（内网/VPN）→ Console:9090 或 Controller:9000
```

公网 **80/443** 应在 Agent 之前终结 TLS；Controller/Console 不对公网裸暴露。

## 3. 控制面 vs 数据面

| 维度 | 控制面 (Controller) | 数据面 (Agent) |
|------|---------------------|----------------|
| 职责 | 配置 CRUD、编译快照、事件 API | 按快照路由、执行检测、反代 |
| 状态 | PostgreSQL + 本地 `data_dir` | **内存中** 当前 `WafEdgeSnapshot` |
| 扩缩 | 多副本 + 无 sticky 要求（无会话状态） | 按 QPS / 副本或每节点 DaemonSet |
| 故障影响 | 无法改策略；见 §5 | 可用**上一版快照**继续转发（见 §5） |

## 4. 配置同步（Agent ← Controller）

Agent 启动后后台循环拉取 **dataplane 快照**（路由 + upstream + 全局 overlay）。

### 4.1 模式

| 环境变量 | 默认 | 说明 |
|----------|------|------|
| `WAF_AGENT_SYNC_MODE` | `watch` | `watch`：长轮询；`poll`：周期全量拉取 |
| `WAF_AGENT_POLL_SEC` | `5` | 仅 `poll` 模式：拉取间隔（秒） |
| `WAF_AGENT_CONTROLLER` | `http://127.0.0.1:9000` | Controller 基址 |

### 4.2 HTTP 接口

| 方法 | 路径 | 用途 |
|------|------|------|
| GET | `/api/waf/edge/watch?since=<gen>&timeout_sec=30` | 长轮询；无变更返回 `unchanged` |
| GET | `/api/waf/edge/dataplane` | 全量快照 |
| POST | `/api/waf/edge/ack` | Agent 确认已应用 `generation` |

### 4.3 流程

```text
1. Agent 启动 → 内存为空快照（default），/health 可用，业务路由可能 502 直到首次同步成功
2. watch 成功 → 替换内存快照 → POST ack(generation)
3. watch 失败 → 降级 GET dataplane 全量拉取
4. 拉取失败 → 打 warn 日志，保留上一版快照，继续服务
5. Controller 长时间不可用 → Agent 持续使用最后成功 generation，不再更新策略
```

实现：`crates/dataplane/nerve-waf-agent/src/dataplane/sync/`。

## 5. 故障域与 HA

### 5.1 Controller 不可用

| 场景 | Agent 行为 | 运维 |
|------|-------------|------|
| 短暂网络抖动 | watch 失败 → 重试 / fallback list | 通常自愈 |
| Controller 宕机 | **继续用最后快照** 转发与检测 | 修复 Controller；Agent 无需重启 |
| 首次启动且从未同步成功 | 无有效路由，代理可能失败 | 先起 Controller + PG + 迁移 |

Controller **多副本**：无内存会话；共用同一 PostgreSQL 即可水平扩展。建议 ≥2 副本 + PDB。

### 5.2 PostgreSQL 不可用

Controller 无法读写策略，health 可能失败；**已运行的 Agent 仍用内存快照**。  
PG 恢复后 Controller 恢复；Agent 在下次 watch/poll 周期获得新快照。

### 5.3 Agent 不可用

该 Agent 前流量中断；其他 Agent 副本不受影响（无共享会话状态）。

详细 K8s 部署拓扑：[ha-deployment.md](./ha-deployment.md)。

## 6. 检测流水线（数据面）

```text
HTTP 请求
  → 匹配路由 / upstream
  → normalize / canonical
  → pipeline: expert (可选) → core (CRS) → custom
  → 裁决 allow / block / log
  → 可选事件写入 Controller（经 API）
  → 反代至 upstream 或返回 block 页
```

代码分层：`crates/engine/`（无 IO）→ `crates/dataplane/`（Agent 嵌入运行时）。

## 7. 持久化与状态

| 数据 | 存储 | 说明 |
|------|------|------|
| 站点、模块、规则元数据 | PostgreSQL | 迁移：`scripts/apply-migrations.sh` |
| 运行时文件、规则缓存 | `NERVE_WAF_DATA` | 卷持久化 |
| 日志 | `logging.*` 配置路径 | 见 [observability.md](./observability.md) |
| 数据面路由快照 | Agent 内存 | 由 Controller 生成 generation |

## 8. 代码仓库结构

```text
Nerve-waf/Nerve-waf/
├── crates/
│   ├── contract/      跨层类型
│   ├── engine/        检测引擎（无 DB/HTTP）
│   ├── controlplane/  Controller API + Store
│   └── dataplane/     Agent + embed
├── apps/console/      Console 前端（可选）
├── deploy/            Docker / Helm
└── docs/              本文档体系
```

详见 [crates/README.md](../crates/README.md)。

## 9. 相关文档

| 文档 | 内容 |
|------|------|
| [concepts/boundaries.md](./concepts/boundaries.md) | 产品边界与非目标 |
| [ha-deployment.md](./ha-deployment.md) | K8s HA 与 Agent 拓扑选型 |
| [configuration.md](./configuration.md) | 环境变量与 config.yml |
| [api.md](./api.md) / [openapi.yaml](./openapi.yaml) | HTTP API |
| [../deploy/TOPOLOGY.md](../deploy/TOPOLOGY.md) | Compose 形态 |
