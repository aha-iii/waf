# Nerve WAF 文档

文档按 **开始 → 概念 → 任务 → 参考 → 运维** 组织，便于按角色查阅。

## 开始（建议阅读顺序）

| # | 文档 | 说明 |
|---|------|------|
| 1 | [what-is-waf.md](./what-is-waf.md) | **是什么** — 定位、三进程、平台边界 |
| 2 | [capabilities.md](./capabilities.md) | **能做什么** — 能力表（含规划状态） |
| 3 | [how-it-works.md](./how-it-works.md) | **怎么工作** — 请求路径、流水线、快照同步 |
| 4 | [getting-started.md](./getting-started.md) | 装后验证 |
| 5 | [installation.md](./installation.md) | 安装路径选择 |

## Concepts（概念）

| 文档 | 说明 |
|------|------|
| [architecture-overview.md](./architecture-overview.md) | 运行时架构（同步、HA、故障域） |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | 代码分层与 crate |
| [concepts/boundaries.md](./concepts/boundaries.md) | 产品边界与非目标 |
| [ha-deployment.md](./ha-deployment.md) | K8s HA |
| [../deploy/TOPOLOGY.md](../deploy/TOPOLOGY.md) | Compose 拓扑 |
| [PORTS.md](./PORTS.md) | 端口 |
| [DEPENDENCIES.md](./DEPENDENCIES.md) | 可选 feature |

## Tasks（任务）

| 文档 | 说明 |
|------|------|
| [../deploy/START.md](../deploy/START.md) | 启停 |
| [../UPGRADING.md](../UPGRADING.md) | 升级 |
| [START-WAF-GATEWAY.md](./START-WAF-GATEWAY.md) | Gateway 联调 |

## Reference（参考）

| 文档 | 说明 |
|------|------|
| [configuration.md](./configuration.md) | 配置与环境变量 |
| [api.md](./api.md) | API 概览 |
| [openapi.yaml](./openapi.yaml) | OpenAPI 3.0 |

## Operations（运维）

| 文档 | 说明 |
|------|------|
| [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md) | 生产 Runbook |
| [observability.md](./observability.md) | 日志与探针 |
| [security-model.md](./security-model.md) | 安全模型 |
| [../CHANGELOG.md](../CHANGELOG.md) | 变更记录 |

## 版本

[VERSION](../VERSION) · [CHANGELOG](../CHANGELOG.md)
