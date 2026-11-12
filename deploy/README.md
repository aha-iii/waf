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
git clone <your-repo-url> nerve-waf
cd nerve-waf/deploy
bash install.sh          # 一键：依赖自检 → .env → 构建启动 → 健康检查 → 冒烟
```

手动分步等价命令：

```bash
cp .env.example .env
bash scripts/up.sh
bash ../scripts/smoke-test.sh

停止：`bash scripts/up.sh --down` 或 `bash install.sh --down`

## 文件说明

| 路径 | 用途 |
|------|------|
| `docker-compose.yml` | 开发栈：PG + 迁移 + Controller + Console + Agent |
| `docker-compose.standalone.yml` | 无 `Nerve-ui` 时的 overlay（无 Console） |
| `docker-compose.prebuilt.yml` | 预构建镜像 + 外置 PG |
| `install.sh` / `install.ps1` | 一键安装（自检 + 启动 + 健康检查 + 冒烟） |
| `scripts/up.sh` / `up.ps1` | 启动/停止（自动探测 Console 源码，无则降级 standalone） |
| `.env.example` | Compose 环境变量（复制为 `.env`） |
| `config/config.yml` | WAF 示例配置 |
| `docker/` | Dockerfile |
| `helm/nerve-waf/` | Helm Chart（Controller + Agent） |

## 克隆范围

| 你的情况 | Console Docker 镜像 | 命令 |
|----------|---------------------|------|
| 有前端源码（设 `NERVE_UI_DIR`，或放在同级 `Nerve-ui/`） | ✅ | `bash scripts/up.sh` |
| 仅本仓库（默认，无前端源码） | ❌ | `bash install.sh`（自动降级 standalone） |

端口：[../docs/PORTS.md](../docs/PORTS.md)。

## 安全提示

Compose 示例使用开发用数据库密码与 `NERVE_AUTH_ENABLED=0`，**不可直接用于生产**。上线清单见 [PRODUCTION.md](../docs/runbooks/PRODUCTION.md)。
