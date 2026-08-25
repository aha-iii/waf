# Nerve WAF 部署

Docker / Helm 示例与配置文件。**克隆仓库后的公开部署入口**。

## 文档

| 文档 | 说明 |
|------|------|
| [../docs/architecture-overview.md](../docs/architecture-overview.md) | **架构总览** |
| [START.md](./START.md) | **快速启动** |
| [TOPOLOGY.md](./TOPOLOGY.md) | 组件、端口、Compose 形态 |
| [helm/nerve-waf/README.md](./helm/nerve-waf/README.md) | Kubernetes Helm |
| [../docs/runbooks/PRODUCTION.md](../docs/runbooks/PRODUCTION.md) | 生产检查与排障 |

## 最快上手

```bash
cd Nerve-waf/Nerve-waf/deploy
cp .env.example .env
bash scripts/up.sh
bash ../scripts/smoke-test.sh

Console（需完整 monorepo 构建）：http://127.0.0.1:9090/

停止：`bash scripts/up.sh --down`

## 文件说明

| 路径 | 用途 |
|------|------|
| `docker-compose.yml` | 开发栈：PG + 迁移 + Controller + Console + Agent |
| `docker-compose.standalone.yml` | 无 `Nerve-ui` 时的 overlay（无 Console） |
| `docker-compose.prebuilt.yml` | 预构建镜像 + 外置 PG |
| `scripts/up.sh` / `up.ps1` | 检测 monorepo 并启动 |
| `.env.example` | Compose 环境变量（复制为 `.env`） |
| `config/config.yml` | WAF 示例配置 |
| `docker/` | Dockerfile |
| `helm/nerve-waf/` | Helm Chart（Controller + Agent） |

## 克隆范围

| 你有的代码 | Console Docker 镜像 | 命令 |
|------------|---------------------|------|
| 完整 monorepo + `Nerve-ui/` | ✅ | `bash scripts/up.sh` |
| 仅 `Nerve-waf/Nerve-waf` | ❌ | 加 `-f docker-compose.standalone.yml` |

端口：[../docs/PORTS.md](../docs/PORTS.md)。

## 安全提示

Compose 示例使用开发用数据库密码与 `NERVE_AUTH_ENABLED=0`，**不可直接用于生产**。上线清单见 [PRODUCTION.md](../docs/runbooks/PRODUCTION.md)。
