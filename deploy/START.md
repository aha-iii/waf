# 快速启动

工作目录：`Nerve-waf/Nerve-waf/deploy`。  
架构说明：[TOPOLOGY.md](./TOPOLOGY.md) · 排障：[../docs/runbooks/PRODUCTION.md](../docs/runbooks/PRODUCTION.md)。

## 1. 前置条件

| 方式 | 需要 |
|------|------|
| Docker Compose | Docker 24+、Compose v2 |
| 含 Console 镜像 | 完整 monorepo（根目录含 `Nerve-ui/`） |
| 仅 API + Agent | 单产品目录 + `docker-compose.standalone.yml` |
| 源码 | WSL/Linux、Rust 1.92、PostgreSQL 14+ |

## 2. Docker Compose（推荐）

### 2.1 完整 monorepo

```bash
git clone <your-repo-url> nerve
cd nerve/Nerve-waf/Nerve-waf/deploy
cp .env.example .env
bash scripts/up.sh
```

| 服务 | 验证 |
|------|------|
| Controller | `curl -fsS http://127.0.0.1:9000/api/health` |
| Console | 浏览器打开 http://127.0.0.1:9090/ |
| Agent | `curl -fsS http://127.0.0.1:9080/health` |

停止：`bash scripts/up.sh --down`

Windows：`powershell -File scripts/up.ps1`

### 2.2 单目录（无 Nerve-ui）

```bash
docker compose -f docker-compose.yml -f docker-compose.standalone.yml up -d --build
```

不启动 Console 容器。需要 UI 时：在 monorepo 内 `cd apps/console && npm run dev`（Node 20+）。

### 2.3 预构建镜像 + 外置数据库

```bash
docker load -i nerve-waf-controller.tar
cp .env.example .env   # 必须填写 DATABASE_URL
docker compose -f docker-compose.prebuilt.yml up -d
```

## 3. 从源码启动

```bash
cd Nerve-waf/Nerve-waf
export DATABASE_URL=postgres://postgres:nerve_waf_dev@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh
cp .env.example .env
bash scripts/start-waf.sh --build
```

详见 [scripts/README.md](../scripts/README.md)（WSL 端口映射、停止进程）。

## 4. 数据库迁移

Compose 栈由 `db-migrate` 服务自动执行一次。手动：

```bash
export DATABASE_URL=postgres://postgres:nerve_waf_dev@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh
```

迁移文件：`crates/controlplane/nerve-waf-store/migrations/`。

## 5. Kubernetes（Helm）

Chart 部署 **Controller + L7 Agent**（values 分 `controller.*` / `agent.*`）。Console 暂不纳入 Chart。

```bash
cd deploy/helm/nerve-waf
helm upgrade --install nerve-waf . -n nerve-waf --create-namespace -f my-values.yaml
kubectl -n nerve-waf port-forward svc/nerve-waf-controller 9000:9000
curl -fsS http://127.0.0.1:9000/api/health
```

详见 [helm/nerve-waf/README.md](./helm/nerve-waf/README.md) · [UPGRADING.md](../UPGRADING.md)。

## 6. 安全（上线前）

| 项 | 开发默认 | 生产 |
|----|----------|------|
| `NERVE_AUTH_ENABLED` | `0`（Compose） | `1` |
| PostgreSQL 密码 | `nerve_waf_dev` | 强密码 |
| `NERVE_API_KEY` | 占位符 | Secret / 强密钥 |

## 7. 常见问题

| 现象 | 处理 |
|------|------|
| 构建失败，找不到 Nerve-ui | 使用 standalone overlay 或克隆完整 monorepo |
| 9000 端口占用 | `deploy/.env` 设 `PORT=9002`，或源码用 `start-waf.sh` |
| Console 404 | 确认走完整 monorepo 构建，访问 :9090 而非 Vite :5177 |
| 更多 | [PRODUCTION.md](../docs/runbooks/PRODUCTION.md) |

## 8. 可选构建

```bash
cargo build -p nerve-waf-controller --release --features full
```

见 [docs/DEPENDENCIES.md](../docs/DEPENDENCIES.md)。
