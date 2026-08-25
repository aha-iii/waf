# Installation

选择安装路径前，先读 [architecture-overview.md](./architecture-overview.md) 了解控制面/数据面分工。

## 决策树

```text
                         ┌─────────────────────────┐
                         │     我要安装 Nerve WAF    │
                         └────────────┬────────────┘
                                      │
            ┌─────────────────────────┼─────────────────────────┐
            │                         │                         │
            ▼                         ▼                         ▼
    ┌───────────────┐        ┌───────────────┐        ┌───────────────┐
    │ 本地 PoC/开发  │        │  Kubernetes   │        │ 仅 API 集成   │
    └───────┬───────┘        └───────┬───────┘        └───────┬───────┘
            │                        │                        │
     Docker Compose              Helm Chart              Helm agent.enabled=false
     scripts/up.sh               ha-deployment.md        POST /api/inspect
            │                        │
     无 UI 构建?                 外置 PostgreSQL
     + standalone.yml            + 迁移脚本
            │                        │
     源码调试                    agent.mode:
     start-waf.sh               deployment | daemonset
```

| 目标 | 推荐路径 | 文档 |
|------|----------|------|
| 5 分钟本地试用 | `deploy/scripts/up.sh` | [deploy/START.md](../deploy/START.md) |
| 无 Nerve-ui / 仅 API+Agent | `docker-compose.standalone.yml` | 下文 §A′ |
| 平台工程 K8s | Helm | [ha-deployment.md](./ha-deployment.md) |
| 贡献者编译 | `make build` + `start-waf.sh` | [CONTRIBUTING.md](../CONTRIBUTING.md) |

## 要求

| 组件 | 版本 |
|------|------|
| PostgreSQL | 14+（16 推荐） |
| Docker | 24+，Compose v2 |
| Rust | 1.92（源码） |
| Kubernetes | 1.24+，Helm 3+ |

## 方式 A — Docker Compose（推荐 PoC）

```bash
git clone <your-repo-url> nerve
cd nerve/Nerve-waf/Nerve-waf/deploy
cp .env.example .env
bash scripts/up.sh
bash ../scripts/smoke-test.sh
```

含 PostgreSQL、迁移、Controller、Agent。Console 容器需 UI 构建链；**可不装 Console**，见 A′。

## 方式 A′ — Standalone（不依赖 Nerve-ui）

```bash
docker compose -f docker-compose.yml -f docker-compose.standalone.yml up -d --build
bash ../scripts/smoke-test.sh
```

## 方式 B — 预构建 OCI 镜像

```bash
docker pull ghcr.io/<org>/nerve-waf:1.0.0
cd deploy && cp .env.example .env
# DATABASE_URL=...  TAG=1.0.0
docker compose -f docker-compose.prebuilt.yml up -d
```

镜像由 tag `nerve-waf-v*` 发布流水线构建（[RELEASE.md](../RELEASE.md)）。

## 方式 C — Kubernetes（Helm）

```bash
export DATABASE_URL=postgres://...
bash scripts/apply-migrations.sh

helm upgrade --install nerve-waf ./deploy/helm/nerve-waf \
  -n nerve-waf --create-namespace \
  -f my-values.yaml
```

生产 values 与 Agent 拓扑：[ha-deployment.md](./ha-deployment.md) · [deploy/helm/nerve-waf/README.md](../deploy/helm/nerve-waf/README.md)

## 方式 D — 源码

```bash
cd Nerve-waf/Nerve-waf
export DATABASE_URL=postgres://postgres:nerve_waf_dev@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh
make build
bash scripts/start-waf.sh
bash scripts/smoke-test.sh
```

## 安装后

1. [getting-started.md](./getting-started.md)  
2. [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)（生产）  
3. [configuration.md](./configuration.md)（调参）
