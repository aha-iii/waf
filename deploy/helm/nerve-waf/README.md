# Nerve WAF Helm Chart

Kubernetes 部署 **Controller（控制面）+ L7 Agent（数据面）**。同一 OCI 镜像，不同 entrypoint。  
架构说明：[docs/architecture-overview.md](../../../docs/architecture-overview.md) · HA：[docs/ha-deployment.md](../../../docs/ha-deployment.md)

## 前置条件

- Kubernetes 1.24+，Helm 3+
- 外置 PostgreSQL；已执行 [scripts/apply-migrations.sh](../../../scripts/apply-migrations.sh)
- 镜像 `ghcr.io/<org>/nerve-waf:<tag>`

## 安装

```bash
helm upgrade --install nerve-waf . \
  -n nerve-waf --create-namespace \
  -f my-values.yaml
```

## Values 结构（Cilium 风格）

| 块 | 说明 |
|----|------|
| `controller.*` | API Deployment、Service、PDB |
| `agent.*` | 数据面；`mode: deployment \| daemonset` |
| `database.url` | PostgreSQL 连接串 |
| `auth.*` | API Key Secret |
| `ingress.*` | 可选，暴露 Controller 管理面 |
| `networkPolicy.enabled` | 可选，限制 Agent↔Controller |
| `metrics.serviceMonitor.enabled` | 可选，Prometheus Operator 探活 |

## Agent 模式

| `agent.mode` | 工作负载 | 适用 |
|--------------|----------|------|
| `deployment` | Deployment + Service | 集中式 L7 入口、HPA 扩缩 |
| `daemonset` | DaemonSet + Service | 每节点 Agent，DaemonSet 节点级部署 |

```yaml
agent:
  enabled: true
  mode: daemonset
  service:
    type: ClusterIP
    port: 9080
  env:
    WAF_AGENT_SYNC_MODE: watch
```

## 生产必改

| 键 | 说明 |
|----|------|
| `image.repository` / `tag` | 固定版本，勿 `:latest` 上生产 |
| `database.url` | 外置 PG，SSL 参数 |
| `auth.existingSecret` | `NERVE_API_KEY` |
| `controller.env.NERVE_AUTH_ENABLED` | `"1"` |

## 验证

```bash
kubectl -n nerve-waf port-forward svc/nerve-waf-controller 9000:9000
curl -fsS http://127.0.0.1:9000/api/health
kubectl -n nerve-waf port-forward svc/nerve-waf-agent 9080:9080
curl -fsS http://127.0.0.1:9080/health
```

## 升级 / 卸载

[UPGRADING.md](../../../UPGRADING.md)

```bash
helm uninstall nerve-waf -n nerve-waf
```

## 相关

- [docs/openapi.yaml](../../../docs/openapi.yaml)  
- [docs/runbooks/PRODUCTION.md](../../../docs/runbooks/PRODUCTION.md)  
