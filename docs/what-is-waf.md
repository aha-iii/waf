# 什么是 Nerve WAF

Nerve WAF 是 **L7 Web 应用防火墙**：在 HTTP/S 流量到达业务之前，完成准入、检测与裁决（放行 / 挑战 / 拦截），并通过控制面统一管理策略与站点。

## 解决什么问题

| 场景 | WAF 做什么 |
|------|------------|
| OWASP Top 10、SQLi/XSS 等 | 规则与语义检测，返回 block/challenge |
| CC / 洪泛、恶意 Bot | 准入层限流、Bot/Geo 策略 |
| 多站点 / 多租户 | Controller 持久化站点与模块，Agent 按快照执行 |
| 策略变更 | Console 或 API 改配置 → 快照下发 → Agent 热更新（无需逐台改文件） |
| 旁路集成 | 外部网关 `POST /api/inspect` 调用检测 API |

## 由哪些进程组成

WAF **不是单进程**，而是三个可独立扩缩的角色：

| 进程 | 角色 | 默认端口 | 说明 |
|------|------|----------|------|
| `nerve-waf-controller` | **控制面** | 9000 | API、策略 CRUD、快照生成、事件持久化 |
| `nerve-waf-agent` | **数据面** | 9080 | 接收流量、执行检测、反代上游 |
| `nerve-waf-console` | **管理 UI**（可选） | 9090 | 静态 Console + 反代 Controller |

```text
运维 / API ──► Controller ──► PostgreSQL（站点、规则、事件）
                    │
                    │ watch / dataplane 快照
                    ▼
客户端流量 ──► Agent ──► 上游业务
```

**最小可用栈：** Controller + Agent（+ PostgreSQL）。Console 可选；无 Console 时可用 API 或独立构建的 UI。

## 在 Nerve 平台中的位置

- WAF 负责 **L7 应用层检测**，不替代 Gateway 路由、DDoS 清洗或 K8s CNI。
- 生产常见路径：`Client → [DDoS] → [Gateway] → WAF Agent → 上游`（括号内可选）。
- 平台总览：[Nerve 系统架构](../../../docs/SYSTEM-ARCHITECTURE.md) · 产品边界：[boundaries.md](./concepts/boundaries.md)

## 不是什么

| 非目标 | 说明 |
|--------|------|
| Ingress Controller | Agent 可接在 Ingress 之后，不实现 Ingress 本身 |
| 集群拓扑控制台 | 拓扑在 Platform / Control；WAF Console 管 L7 防护 |
| 全家桶硬依赖 | Gateway、DDoS、Observer 均为可选 sibling |

## 下一步读什么

| 目的 | 文档 |
|------|------|
| 能力清单 | [能做什么](./capabilities.md) |
| 请求怎么走、策略怎么同步 | [怎么工作](./how-it-works.md) |
| 5 分钟试用 | [快速入门](./getting-started.md) · [deploy/START.md](../deploy/START.md) |
| 运行时架构细节 | [architecture-overview.md](./architecture-overview.md) |
