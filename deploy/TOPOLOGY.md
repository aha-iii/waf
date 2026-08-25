# Nerve WAF 部署架构

**完整架构（同步、HA、故障域）：** [../docs/architecture-overview.md](../docs/architecture-overview.md)

产品根目录：`Nerve-waf/Nerve-waf/`。本文描述 **运行时** 组件与 Compose / Helm 形态。

## 组件

| 组件 | 二进制 | 默认端口 | 职责 |
|------|--------|----------|------|
| Controller | `nerve-waf-controller` | **9000** | 控制面 API：策略、规则快照、事件、`/api/inspect` |
| Console | `nerve-waf-console` | **9090** | 管理 UI（静态资源，反代 Controller） |
| Agent | `nerve-waf-agent` | **9080**（HTTPS 可选 9443） | L7 数据面：检测与反代 |

数据持久化：**PostgreSQL**（迁移脚本 `scripts/apply-migrations.sh`）。  
配置：`deploy/config/config.yml` + `deploy/.env`。

```text
运维 / 浏览器
    │
    ├─ :9090 ── nerve-waf-console ──▶ nerve-waf-controller :9000
    │                                      │
    │                                      ├─ PostgreSQL
    │                                      └─ 策略 / 快照下发
    │
客户端 HTTP(S)
    │
    └─ :9080 ── nerve-waf-agent ──▶ 检测流水线 ──▶ 上游业务
                      ▲
                      └── 拉取配置：WAF_AGENT_CONTROLLER → :9000
```

生产建议：Controller / Console 仅内网或 Ingress；Agent 作为流量入口或 Sidecar，公网 **80/443** 由前置 TLS 终结。

## Compose 形态

| 形态 | 文件 | 适用 |
|------|------|------|
| **开发一键栈** | `docker-compose.yml` | 完整 monorepo（须含 `Nerve-ui/`）→ PG + 迁移 + 三进程 |
| **单仓库** | `docker-compose.yml` + `docker-compose.standalone.yml` | 无 UI 构建能力 → 仅 Controller + Agent + PG |
| **预构建镜像** | `docker-compose.prebuilt.yml` | 已 `docker load` 镜像 + **外置 PostgreSQL** |

| Dockerfile | 构建 context | 含 Console |
|------------|--------------|------------|
| `docker/Dockerfile.compose` | monorepo 根 `../../..` | 是 |
| `docker/Dockerfile.standalone` | 产品根 `..` | 否 |
| `docker/Dockerfile.prebuilt` | 发布包内二进制 | 视发布包 |

一键脚本（自动检测 `Nerve-ui`）：

```bash
cd deploy && bash scripts/up.sh
```

## Kubernetes

Chart 路径：`deploy/helm/nerve-waf/`（**Controller + L7 Agent**；可选 Ingress）。

```bash
helm upgrade --install nerve-waf ./deploy/helm/nerve-waf \
  -n nerve-waf --create-namespace
```

安装与 values 说明：[helm/nerve-waf/README.md](./helm/nerve-waf/README.md)。  
上线前设置 `DATABASE_URL`、`NERVE_API_KEY`，`NERVE_AUTH_ENABLED=1`。

## 端口与冲突

| 端口 | 用途 | 常见冲突 |
|------|------|----------|
| 9000 | Controller | 本机 ClickHouse 等 → dev 可设 `PORT=9002` 或 `start-waf.sh` 自动避让 |
| 9080 | Agent | 同机其它服务若占用 9080 → 设 `WAF_AGENT_PORT` |
| 9090 | Console | Prometheus 等 → 设 `CONSOLE_PORT` |

完整说明：[docs/PORTS.md](../docs/PORTS.md)。

## 源码开发（非容器）

```bash
cd Nerve-waf/Nerve-waf
export DATABASE_URL=postgres://postgres:nerve_waf_dev@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh
bash scripts/start-waf.sh --build
```

Windows + WSL：`scripts/start-waf.ps1 -PortForward -Build`。

## 文档索引

- [START.md](./START.md) — 逐步启动
- [README.md](./README.md) — deploy 目录说明
- [../README.md](../README.md) — 产品总览
- [../docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) — 代码分层
- [../docs/runbooks/PRODUCTION.md](../docs/runbooks/PRODUCTION.md) — 生产排障
