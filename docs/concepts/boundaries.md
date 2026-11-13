# Product boundaries

Nerve WAF **开源仓库**交付 L7 检测、控制面 API、L7 Agent 与可选 Console。  
下列能力**不属于本仓库的默认交付或硬依赖**。

## 本仓库负责

| 能力 | 组件 |
|------|------|
| L7 HTTP/S 检测（CRS / 自定义 / 可选 Expert） | Controller + Agent |
| 策略与站点管理 API | Controller |
| 管理 Console（可选） | Console |
| 数据面路由快照下发 | Controller → Agent |

## 非目标（Non-goals）

| 能力 | 说明 |
|------|------|
| **Kubernetes CNI / Pod 网络** | 不在此仓库；可选 sibling `Nerve-cni` |
| **集群拓扑 / NetworkPolicy 控制台** | 不在此仓库；可选 `Nerve-control` |
| **节点 eBPF L3/L4 执行** | 不在此仓库；可选 `Nerve/` 节点 Agent |
| **L7 网关 / 入口路由产品** | 不在此仓库；可选 `Nerve-gateway` |
| **替代 Ingress Controller** | WAF Agent 可放在 Ingress 之后，但不实现 Ingress 本身 |
| **Prometheus metrics（当前版本）** | 规划中；现用 health + 日志，见 [observability.md](../observability.md) |

## 可选集成

| 集成 | 方式 |
|------|------|
| Expert 规则引擎 | 构建 `--features expert`（依赖 `github.com/ahaiii/antibody`） |
| 告警中心 | `NERVE_ALERT_ENABLED`，HTTP 至 Nerve-alert |
| 节点 L3 deny | `NERVE_URL` 指向节点 Agent API（**可选** env） |
| HTTP 旁路检测 | 外部组件 `POST /api/inspect` |

集成不应在部署 Nerve WAF 最小栈时**强制**安装上述 sibling。

## 遗留开关（勿在生产启用）

| 环境变量 | 说明 |
|----------|------|
| `WAF_ENABLE_LEGACY_TOPOLOGY=1` | 紧急回滚：拓扑回 WAF API |
| `WAF_ENABLE_LEGACY_NP=1` | 紧急回滚：NP 回 WAF API |

生产保持 `0`。平台拓扑请用独立控制面产品。

## 阅读顺序

1. [architecture-overview.md](../architecture-overview.md) — 运行时架构  
2. [installation.md](../installation.md) — 如何安装**仅 WAF**  
3. [ha-deployment.md](../ha-deployment.md) — K8s 生产拓扑  
