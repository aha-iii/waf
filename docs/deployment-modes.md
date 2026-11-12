# 部署模式选型

同一套控制面与检测能力，可按环境选择不同的**数据面部署形态**（Compose 或 Kubernetes）。

## 两种模式对比

| 模式 | 适用场景 | 数据面形态 | 入口 |
|------|----------|------------|------|
| **Compose** | 本地 PoC、CI、功能验证 | Docker Compose 多容器 | [安装指南 · Compose](/waf/installation) |
| **Kubernetes** | 生产集群、HA、GitOps | Helm：Deployment / DaemonSet Agent | [Helm Chart](/waf/deploy/helm) |

## Compose 模式

- **控制面 + Agent + PostgreSQL** 在一台宿主机或单机 VM 上快速拉起。
- 可选 Console（9090）；无 UI 时用 standalone Compose 或纯 API。
- 典型路径：[deploy/START.md](../deploy/START.md) → [getting-started.md](./getting-started.md)。

## Kubernetes 模式

- Controller、Agent、Console 分 Chart 部署；Agent 可按节点 DaemonSet 或按工作负载 Deployment。
- 外置 PostgreSQL、迁移脚本、资源与探针见 [ha-deployment.md](./ha-deployment.md)。
- 升级与版本矩阵见 [UPGRADING.md](../UPGRADING.md)。

## 与 DDoS 串联（可选）

生产常见 `Client → DDoS Edge → WAF Agent → 上游`。两产品独立部署，无硬依赖。见 [DDoS 部署](/ddos/deployment)。

## 如何选择

```text
需要 5 分钟内验证？ ──► Compose
需要生产 HA / 多副本？ ──► Kubernetes + Helm
需要入口清洗 + L7 检测？ ──► 先 DDoS，再 WAF（分 Chart）
仅要检测 API、无 Agent 反代？ ──► Helm agent.enabled=false + /api/inspect
```

## 下一步

| 目标 | 文档 |
|------|------|
| 快速入门 | [getting-started.md](./getting-started.md) |
| 安装细节 | [installation.md](./installation.md) |
| 架构理解 | [architecture-overview.md](./architecture-overview.md) |
