# High availability and Kubernetes deployment

面向平台/SRE：Controller、Agent 在 Kubernetes 上的推荐拓扑与选型。

## 1. 组件 HA 摘要

| 组件 | 状态 | HA 策略 |
|------|------|---------|
| **PostgreSQL** | 外部托管 | RDS / Cloud SQL / Patroni；**不由本 Chart 部署** |
| **Controller** | 无本地会话 | `replicaCount ≥ 2`，共用 PG，PDB `minAvailable: 1` |
| **Agent** | 内存快照 | 多副本或 DaemonSet；**无 Pod 间共享状态** |

Controller 宕机时 Agent **继续服务最后快照**（见 [architecture-overview.md](./architecture-overview.md) §5）。

## 2. Agent 部署模式选型

Helm `agent.mode`：

| 模式 | 适用 | 说明 |
|------|------|------|
| **`deployment`**（默认） | 集中式 L7 入口、少量大流量 Pod | `replicaCount` + `Service` LoadBalancer/ClusterIP |
| **`daemonset`** | 每节点流量、NodePort/HostNetwork 场景 | 每 Node 一个 Agent，DaemonSet 节点级部署 |

```yaml
agent:
  enabled: true
  mode: deployment   # or daemonset
  replicaCount: 2    # deployment only
  service:
    type: LoadBalancer   # 或 ClusterIP + Ingress 指到 Agent
```

### 2.1 推荐参考拓扑

**拓扑 A — 集中式 Agent（中小规模）**

```text
Ingress/LB → Service(nerve-waf-agent) → Deployment(replicas=N) → upstream
                ↑
         Controller(×2) → PostgreSQL
```

**拓扑 B — 每节点 Agent（大规模 / 低延迟）**

```text
Node traffic → DaemonSet(nerve-waf-agent) → upstream
                      ↑ watch
               Controller(×2) → PostgreSQL
```

**拓扑 C — 仅 API（无数据面）**

```yaml
agent:
  enabled: false
```

用于外部系统仅调用 `/api/inspect`、不自管 L7 反代。

## 3. Helm 安装示例（生产向）

```yaml
# prod-values.yaml
image:
  repository: ghcr.io/YOUR-ORG/nerve-waf
  tag: "1.0.0"

database:
  url: "postgres://user:pass@pg.example:5432/nerve?sslmode=require"

auth:
  enabled: true
  existingSecret: nerve-waf-auth

controller:
  replicaCount: 2
  env:
    NERVE_AUTH_ENABLED: "1"

agent:
  enabled: true
  mode: deployment
  replicaCount: 3
  service:
    type: LoadBalancer

networkPolicy:
  enabled: true

ingress:
  enabled: true
  className: nginx
  hosts:
    - host: waf-admin.example.com
      paths:
        - path: /
          pathType: Prefix
          service: controller
```

```bash
# 先迁移
export DATABASE_URL=postgres://...
bash scripts/apply-migrations.sh

helm upgrade --install nerve-waf ./deploy/helm/nerve-waf \
  -n nerve-waf -f prod-values.yaml
```

## 4. 升级与滚动

1. `apply-migrations.sh`  
2. `helm upgrade` Controller 先于或并行 Agent（Agent 兼容旧 generation）  
3. `kubectl rollout status`  
4. `bash scripts/smoke-test.sh`（port-forward 后）

见 [UPGRADING.md](../UPGRADING.md)。

## 5. 资源起点（需按流量压测调整）

| 组件 | requests | limits |
|------|----------|--------|
| Controller | 200m CPU, 256Mi | 1Gi mem |
| Agent | 100m CPU, 128Mi | 512Mi mem |

高 QPS 场景优先水平扩展 Agent 副本数。

## 6. 相关文档

- [architecture-overview.md](./architecture-overview.md)  
- [deploy/helm/nerve-waf/README.md](../deploy/helm/nerve-waf/README.md)  
- [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)  
