# Nerve WAF 怎么工作

本文说明 **一条请求如何被处理**、**策略如何到达 Agent**，以及 **检测流水线** 如何裁决。运行时细节与故障域见 [architecture-overview.md](./architecture-overview.md)；代码 crate 分层见 [ARCHITECTURE.md](./ARCHITECTURE.md)。

## 1. 总体流程（一图读懂）

```text
┌─────────────┐     管理路径      ┌──────────────────┐     SQL      ┌────────────┐
│ Console/API │ ───────────────► │ nerve-waf-       │ ◄──────────► │ PostgreSQL │
└─────────────┘                  │ controller       │              └────────────┘
                                 └────────┬─────────┘
                                          │ ① 编译 WafEdgeSnapshot
                                          │ ② GET watch / dataplane
                                          ▼
┌─────────────┐   业务流量 :9080   ┌──────────────────┐   ③ inspect   ┌─────────┐
│   Client    │ ────────────────► │ nerve-waf-agent  │ ─────────────► │ 上游    │
└─────────────┘                  └──────────────────┘                └─────────┘
                                          │
                                          └──► ④ 事件写回 Controller（若命中）
```

| 步骤 | 发生什么 |
|------|----------|
| ① | 运维在 Console/API 改站点、模块、规则 → Controller 持久化并 bump snapshot `generation` |
| ② | Agent 后台 `watch` 长轮询或 `poll` 拉取全量快照，写入内存，POST `ack` |
| ③ | 每个请求：匹配路由 → 跑检测流水线 → allow / challenge / block → 或反代上游 |
| ④ | 拦截/观察事件异步持久化，供日志与研判 |

## 2. 数据面：单请求路径

```text
HTTP Request
  → 路由匹配（快照中的 site / upstream）
  → pre_access（CC / Geo / Bot 等准入，可单独拒绝）
  → expert（可选 Arc-Cortex / Expert 引擎）
  → core（CRS + 语义增强等）
  → custom（自定义 Lua / 规则）
  → 裁决：pass · challenge · block
  → 转发上游 或 返回拦截页
```

**短路规则：** 任一段返回 `InspectHit` 即 `finish`，后续段不再执行。

Expert 未启用或 pack 加载失败时，**core 仍兜底**（生产勿只开 expert 且无 pack）。

## 3. 控制面：快照同步

Agent 启动后循环同步（实现：`crates/dataplane/nerve-waf-agent/src/dataplane/sync/`）。

| 模式 | 环境变量 | 行为 |
|------|----------|------|
| watch（默认） | `WAF_AGENT_SYNC_MODE=watch` | 长轮询 `/api/waf/edge/watch?since=<gen>` |
| poll | `WAF_AGENT_SYNC_MODE=poll` | 每 `WAF_AGENT_POLL_SEC` 秒全量 GET dataplane |

| API | 用途 |
|-----|------|
| `GET /api/waf/edge/watch` | 有变更返回新快照；无变更 `unchanged` |
| `GET /api/waf/edge/dataplane` | 全量快照 |
| `POST /api/waf/edge/ack` | Agent 确认已应用 `generation` |

**故障行为：**

- 拉取失败 → 打日志，**保留上一版快照**继续服务（策略不更新，但不裸奔停服）
- Controller 长时间不可用 → Agent 用最后成功 generation，/health 仍可用

## 4. 控制面 vs 数据面

| 维度 | Controller | Agent |
|------|------------|-------|
| 状态 | PostgreSQL + `data_dir` | 内存中的当前快照 |
| 扩缩 | 多副本，无会话 sticky | 按 QPS 水平扩或 DaemonSet |
| 公网暴露 | **不应**对公网裸暴露 | 承接 9080 业务入口（TLS 通常在前面终止） |

## 5. 可选：旁路检测

外部组件不经过 Agent 反代，可直接调用：

```http
POST /api/inspect
Content-Type: application/json
X-API-Key: <key>
```

Controller 在控制面执行同一套 inspect 逻辑，返回 `action` / 命中详情。适用于 Gateway remote 模式或自定义接入。

## 6. 与 Expert（Arc-Cortex）的关系

| 层 | 位置 | 作用 |
|----|------|------|
| L0 准入 | Agent / 模块 | CC、Geo、Bot（在 expert 之前） |
| Expert 段 | `nerve-waf-inspect` pipeline | 调用 Antibody Engine（若编译启用） |
| Core 段 | 同上 | CRS、libinjection 等 |
| Fusion / RIS | Antibody 内核 | 路径选择与 block 资格，见 Antibody 文档 |

引擎内核：[Nerve-antibody/docs/SYSTEM-ARCHITECTURE-zh.md](../../Nerve-antibody/docs/SYSTEM-ARCHITECTURE-zh.md)

## 7. 部署形态对流程的影响

| 形态 | 流量路径 |
|------|----------|
| **最小 Compose** | Client → Agent:9080 → 上游；Console:9090 管理 |
| **K8s Helm** | Ingress/LB → Agent Service → upstream；Controller Deployment |
| **+ Gateway** | Client → Gateway → WAF Agent（插件）→ 上游 |

拓扑图：[deploy/TOPOLOGY.md](../deploy/TOPOLOGY.md) · HA：[ha-deployment.md](./ha-deployment.md)

## 下一步

| 目的 | 文档 |
|------|------|
| 装一套验证 | [getting-started.md](./getting-started.md) |
| 配置项 | [configuration.md](./configuration.md) |
| 生产 Runbook | [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md) |
