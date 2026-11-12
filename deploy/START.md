# 快速启动

工作目录：`deploy/`（仓库根下）。  
架构说明：[TOPOLOGY.md](./TOPOLOGY.md) · 排障：[../docs/runbooks/PRODUCTION.md](../docs/runbooks/PRODUCTION.md)。

## 1. 前置条件

| 方式 | 需要 |
|------|------|
| Docker Compose | Docker 24+、Compose v2 |
| 含 Console 镜像 | 完整 monorepo（根目录含 `Nerve-ui/`） |
| 仅 API + Agent | 单产品目录 + `docker-compose.standalone.yml` |
| 源码 | WSL/Linux、Rust 1.92、PostgreSQL 14+ |

## 2. Docker Compose（推荐）

### 2.1 一键安装

```bash
cd deploy
bash install.sh
```

自动完成：依赖自检 → 生成 `.env` → 构建启动 → 等健康 → 冒烟。安装后打印实际端口地址。

| 服务 | 验证 |
|------|------|
| Controller | `curl -fsS http://127.0.0.1:9000/api/health` |
| Console | 浏览器打开 http://127.0.0.1:9090/ |
| Agent | `curl -fsS http://127.0.0.1:9080/health` |

停止：`bash install.sh --down` · 其他参数：`bash install.sh --help`

Windows：`powershell -ExecutionPolicy Bypass -File install.ps1`

### 2.2 无 Console UI（standalone）

独立仓库默认没有 `Nerve-ui` 前端源码，`install.sh` 会自动降级为 standalone（仅 Controller + Agent）。
也可显式指定：

```bash
bash install.sh --standalone
```

需要 Console 时，指向前端源码目录后重跑：

```bash
NERVE_UI_DIR=/path/to/Nerve-ui bash install.sh
```

### 2.3 预构建镜像 + 外置数据库

```bash
docker load -i nerve-waf-controller.tar
cp .env.example .env   # 必须填写 DATABASE_URL
docker compose -f docker-compose.prebuilt.yml up -d
```

## 3. 从源码启动

```bash
cd nerve-waf
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
| 提示未找到 Nerve-ui | 正常：独立仓库无前端源码，已自动降级 standalone；需要 UI 时设 `NERVE_UI_DIR` |
| 9000 端口占用 | `bash install.sh --port 9100`，或 `deploy/.env` 设 `PORT=9002` |
| Console 404 | standalone 模式不含 Console；完整栈访问 :9090（不是 Vite :5178） |
| **检测全部 allow，攻击没被拦** | 站点未绑定防护策略。绑定 `tpl-standard` 后 `custom-rules/reload`，见 [installation.md §关键一步](../docs/installation.md) |
| curl 访问 127.0.0.1 返回 502 | 系统级 `HTTP_PROXY` 拦截了本地回环，加 `--noproxy '*'` |
| 更多 | [PRODUCTION.md](../docs/runbooks/PRODUCTION.md) |

## 8. 可选构建

```bash
cargo build -p nerve-waf-controller --release --features full
```

见 [docs/DEPENDENCIES.md](../docs/DEPENDENCIES.md)。
