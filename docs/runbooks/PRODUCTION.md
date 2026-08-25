# Nerve WAF 部署与运维

面向 **nerve-waf-controller / nerve-waf-agent** 上线与排障。  
**架构与故障行为：** [architecture-overview.md](../architecture-overview.md) · 安装：[deploy/START.md](../../deploy/START.md)

## 健康检查

```bash
curl -fsS "http://${WAF_CONTROLLER:-127.0.0.1:9000}/api/health"
curl -fsS "http://${WAF_AGENT:-127.0.0.1:9080}/health"
curl -fsS -o /dev/null -w '%{http_code}\n' "http://${WAF_CONSOLE:-127.0.0.1:9090}/"
```

Kubernetes（Helm Chart：**Controller + Agent**，见 [deploy/helm/nerve-waf/README.md](../../deploy/helm/nerve-waf/README.md)）：

```bash
kubectl -n "${NS:-nerve-waf}" get pods -l app.kubernetes.io/name=nerve-waf
kubectl -n "${NS:-nerve-waf}" logs deploy/nerve-waf-controller --tail=100
kubectl -n "${NS:-nerve-waf}" logs deploy/nerve-waf-agent --tail=100
kubectl -n "${NS:-nerve-waf}" port-forward svc/nerve-waf-controller 9000:9000
kubectl -n "${NS:-nerve-waf}" port-forward svc/nerve-waf-agent 9080:9080
```

## 上线前检查

| 项 | 要求 |
|----|------|
| `DATABASE_URL` | 可达；已执行 `scripts/apply-migrations.sh` |
| `NERVE_AUTH_ENABLED` | 生产为 `1` |
| `NERVE_API_KEY` | 强密钥；Helm 使用 Secret |
| `WAF_ENABLE_LEGACY_TOPOLOGY` | 保持 `0` |
| `WAF_ENABLE_LEGACY_NP` | 保持 `0` |
| 公网暴露 | 经 TLS / Ingress；避免裸暴露 Agent 9080 |

## 端口

| 进程 | 默认 | 环境变量 |
|------|------|----------|
| Controller | 9000 | `PORT` |
| Console | 9090 | `CONSOLE_PORT` / `WAF_CONSOLE_PORT` |
| Agent | 9080 | `WAF_AGENT_PORT` |

同机端口冲突时修改对应 env；详见 [PORTS.md](../PORTS.md)。

## 常见故障

### Controller 无法启动 / health 失败

1. 检查 `DATABASE_URL` 与迁移是否完成  
2. 查看日志：`RUST_LOG=info`  
3. 确认 PostgreSQL 连接数、SSL 参数与 URL 一致  

### Agent 不同步策略

1. `WAF_AGENT_CONTROLLER` 指向可达 Controller  
2. Controller health 正常；检查 NetworkPolicy 是否放行 Agent→Controller:9000  
3. 查看 Agent 日志：`dataplane loaded` / `watch failed` / `dataplane refresh`  
4. **Controller 长期宕机时 Agent 仍用最后快照**——策略不会更新，见 [architecture-overview.md](../architecture-overview.md) §5

### Console 空白或 404

1. Docker 完整栈需 monorepo 构建（含 Console `dist/`）  
2. 确认 `WAF_CONTROLLER_UPSTREAM` 指向正确 Controller  
3. 访问 Console 端口 9090，不是 Vite dev 5177  

### `docker compose up --build` 失败（无 Console）

缺少 `Nerve-ui` → 使用 `docker-compose.standalone.yml`，或克隆完整 monorepo。

## 数据与日志（容器内）

| 路径 | 内容 |
|------|------|
| `/var/lib/nerve-waf/data` | 运行时数据、规则快照 |
| `/var/lib/nerve-waf/logs` | 应用日志 |
| `/opt/nerve-waf/console` | Console 静态文件 |

Compose 卷名：`nerve-waf-data`、`nerve-waf-logs`；形态 A 另有 `nerve-waf-pg`。

## 回滚

- **镜像**：Helm `rollback` 或 Compose 指定上一 `TAG`  
- **配置**：Console 快照 / 版本管理 `deploy/config/config.yml`  
- **`WAF_ENABLE_LEGACY_*=1`**：仅紧急临时手段，勿作为长期方案  

## 相关文档

- [deploy/START.md](../../deploy/START.md)  
- [deploy/TOPOLOGY.md](../../deploy/TOPOLOGY.md)  
- [ARCHITECTURE.md](../ARCHITECTURE.md)  
- [PORTS.md](../PORTS.md)  
