# Nerve WAF

> L7 Web Application Firewall — inspection engine, control API, and L7 agent dataplane.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-1.0.0-green.svg)](VERSION)

<!-- CI badge: replace YOUR-ORG/YOUR-REPO after publishing to GitHub -->
[![Nerve WAF CI](https://github.com/YOUR-ORG/YOUR-REPO/actions/workflows/nerve-waf-ci.yml/badge.svg)](https://github.com/YOUR-ORG/YOUR-REPO/actions/workflows/nerve-waf-ci.yml)

Nerve WAF 对 HTTP/S 流量做规范化与规则检测（CRS / 自定义 / 可选 Expert 引擎），并提供策略与快照下发。  
仓库路径：`Nerve-waf/Nerve-waf`（monorepo 内）· crate：`nerve-waf-*`

**架构总览（建议先读）：** [docs/what-is-waf.md](./docs/what-is-waf.md) → [capabilities.md](./docs/capabilities.md) → [how-it-works.md](./docs/how-it-works.md) → [architecture-overview.md](./docs/architecture-overview.md)

## 组件

| 组件 | 端口 | 二进制 |
|------|------|--------|
| Controller（控制面） | 9000 | `nerve-waf-controller` |
| Agent（数据面） | 9080 | `nerve-waf-agent` |
| Console（可选） | 9090 | `nerve-waf-console` |

## 环境要求

| 依赖 | 版本 |
|------|------|
| PostgreSQL | 14+（推荐 16） |
| Docker + Compose | 24+ / Compose v2（容器路径） |
| Rust | 1.92（`rust-toolchain.toml`，源码路径） |
| Kubernetes + Helm | 1.24+ / Helm 3+（K8s 路径） |

## 安装怎么选？

```text
                    ┌─ PoC / 本地 ──► deploy/scripts/up.sh (Compose)
                    │
  我要装 Nerve WAF ─┼─ 开发调试 ──► scripts/start-waf.sh (源码三进程)
                    │
                    ├─ K8s 生产 ──► deploy/helm/nerve-waf (Controller+Agent)
                    │
                    └─ 已有镜像 ──► docker-compose.prebuilt.yml + DATABASE_URL
```

| 你的场景 | 文档 |
|----------|------|
| 第一次安装 | [docs/installation.md](./docs/installation.md) |
| 装完验证 | [docs/getting-started.md](./docs/getting-started.md) + `scripts/smoke-test.sh` |
| K8s HA / Agent 选型 | [docs/ha-deployment.md](./docs/ha-deployment.md) |
| 产品边界 | [docs/concepts/boundaries.md](./docs/concepts/boundaries.md) |

## 快速开始（Docker）

```bash
git clone <your-repo-url> nerve
cd nerve/Nerve-waf/Nerve-waf/deploy
cp .env.example .env
bash scripts/up.sh
bash ../scripts/smoke-test.sh
```

无 Console UI 构建链时（**不依赖 Nerve-ui**）：

```bash
docker compose -f docker-compose.yml -f docker-compose.standalone.yml up -d --build
bash ../scripts/smoke-test.sh
```

## 文档

| 文档 | 说明 |
|------|------|
| [docs/README.md](./docs/README.md) | **文档索引** |
| [docs/architecture-overview.md](./docs/architecture-overview.md) | 架构（控制/数据面、同步、故障域） |
| [docs/installation.md](./docs/installation.md) | 安装方式 |
| [docs/api.md](./docs/api.md) · [openapi.yaml](./docs/openapi.yaml) | API |
| [docs/security-model.md](./docs/security-model.md) | 安全模型 |
| [deploy/helm/nerve-waf/README.md](./deploy/helm/nerve-waf/README.md) | Helm |
| [CHANGELOG.md](./CHANGELOG.md) · [UPGRADING.md](./UPGRADING.md) | 版本 |

## 发布物

| 类型 | 坐标 |
|------|------|
| OCI 镜像 | `ghcr.io/<org>/nerve-waf:1.0.0`（tag `nerve-waf-v1.0.0`） |
| Helm Chart | `deploy/helm/nerve-waf/` |

发版流程：[RELEASE.md](./RELEASE.md)

## 社区

- [CONTRIBUTING.md](./CONTRIBUTING.md)
- [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md)
- [SECURITY.md](./SECURITY.md)

## License

[MIT](./LICENSE)
