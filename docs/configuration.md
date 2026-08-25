# Configuration reference

优先级：**CLI 参数 > 环境变量 > `config.yml`**（见 `.env.example` 注释）。

配置文件样例：`deploy/config/config.yml`。Compose 挂载到容器 `/etc/nerve-waf/config.yml`。

## 环境变量（常用）

| 变量 | 默认 | 组件 | 说明 |
|------|------|------|------|
| `PORT` | `9000` | Controller | HTTP API 端口 |
| `WAF_CONSOLE_PORT` / `CONSOLE_PORT` | `9090` | Console | UI 端口 |
| `WAF_AGENT_PORT` | `9080` | Agent | L7 数据面 |
| `DATABASE_URL` | — | Controller | PostgreSQL 连接串（**必填**生产） |
| `WAF_CONFIG` | `./config.yml` | Controller | YAML 配置路径 |
| `NERVE_WAF_DATA` | `./data` | Controller | 运行时数据目录 |
| `NERVE_AUTH_ENABLED` | `1`（源码） / `0`（Compose dev） | Controller | API 鉴权 |
| `NERVE_API_KEY` | — | Controller | API Key（`NERVE_AUTH_ENABLED=1` 时） |
| `WAF_AGENT_CONTROLLER` | `http://127.0.0.1:9000` | Agent | Controller 基址 |
| `WAF_AGENT_SYNC_MODE` | `watch` | Agent | `watch`（长轮询）或 `poll`（周期全量） |
| `WAF_AGENT_POLL_SEC` | `5` | Agent | `poll` 模式拉取间隔（秒） |
| `NERVE_EXPERT_ENGINE` | `off` | Controller/Agent | 启用 Expert（需 `--features expert` 构建） |
| `NERVE_ALERT_ENABLED` | `0` | Controller | 告警 dispatch |
| `RUST_LOG` | `info` | 全部 | 日志级别 |
| `WAF_ENABLE_LEGACY_TOPOLOGY` | `0` | Controller | 紧急回滚，生产保持 `0` |
| `WAF_ENABLE_LEGACY_NP` | `0` | Controller | 紧急回滚，生产保持 `0` |

完整端口说明：[PORTS.md](./PORTS.md)。

## `config.yml` 结构

```yaml
waf:
  api:
    port: 9000              # 可被 PORT 覆盖
  data_dir: "/var/lib/nerve-waf/data"
  plugins_dir: "/opt/nerve-waf/plugins"
  nerve_url: "http://127.0.0.1:8766"   # 可选节点 Agent API
  database:
    url: "postgres://..."   # 可被 DATABASE_URL 覆盖
  console:
    port: 9090              # 生产 Console；Vite dev 常用 5177

logging:
  level: info
  access_log: "/var/lib/nerve-waf/logs/access.log"
  audit_log: "/var/lib/nerve-waf/logs/audit.log"
  error_log: "/var/lib/nerve-waf/logs/error.log"
  server_log: "/var/lib/nerve-waf/logs/server.log"
  format: json              # json | text
  rotate: daily
  max_size_mb: 100
  max_age_days: 7
  max_backups: 14
```

## Agent 同步与 Controller 故障

详见 [architecture-overview.md](./architecture-overview.md) §4–§5。

| 情况 | Agent 行为 |
|------|------------|
| 首次启动，尚未同步成功 | `/health` 正常；业务路由可能不可用 |
| 同步成功后再断 Controller | **保留最后 generation 快照**，继续转发与检测 |
| `watch` 失败 | 自动 fallback `GET /api/waf/edge/dataplane` |
| 连续拉取失败 | 日志 `dataplane refresh` warning；不清空内存快照 |

## Helm

Values 映射见 [deploy/helm/nerve-waf/README.md](../deploy/helm/nerve-waf/README.md) 与 `values.yaml`。

## 生产建议

1. `DATABASE_URL` 使用 Secret（Helm `auth.existingSecret` / K8s Secret）  
2. `NERVE_AUTH_ENABLED=1`，轮换 `NERVE_API_KEY`  
3. 勿在公网裸暴露 Agent `9080`；Controller/Console 走 Ingress/VPN  
4. 见 [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)
