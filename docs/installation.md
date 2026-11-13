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

### 资源建议

| 规模 | CPU | 内存 | 磁盘 | 说明 |
|------|-----|------|------|------|
| PoC / 本地 | 2 核 | 4 GB | 10 GB | 含 PostgreSQL 容器 |
| 小规模生产 | 4 核 | 8 GB | 50 GB | 事件保留 30 天 |
| 高吞吐 | 8 核+ | 16 GB+ | 200 GB+ | 按 QPS 水平扩 Agent 副本 |

`install.sh` 会在磁盘低于 5 GB 时给出警告；镜像与数据均落在 Docker 数据目录。

### 端口

| 端口 | 服务 | 说明 |
|------|------|------|
| `9000` | Controller（控制面 API） | 可用 `--port` 或 `.env` 的 `PORT` 修改 |
| `9090` | Console（Web 控制台） | 仅完整栈；standalone 不启动 |
| `9080` | Agent（数据面） | `.env` 的 `WAF_AGENT_PORT` |

端口被占用时 `install.sh` 会直接报错并给出换端口命令，不会静默失败。


## 方式 A — Docker Compose（推荐 PoC）

```bash
git clone <your-repo-url> nerve-waf
cd nerve-waf/deploy
bash install.sh
```

`install.sh` 依次完成：依赖自检（Docker 24+ / Compose v2）→ 生成 `.env` → 构建并启动 → 等待健康 → 冒烟验证，最后打印访问地址。常见参数：

| 参数 | 作用 |
|------|------|
| `--standalone` | 强制仅 Controller + Agent（无 Console） |
| `--full` | 要求 Console UI 源码，缺失即失败（默认自动降级） |
| `--no-build` | 复用已有镜像，不重新构建 |
| `--skip-smoke` | 跳过冒烟测试 |
| `--port N` | 覆盖 Controller 端口 |
| `--timeout N` | 健康检查超时秒数（默认 180） |
| `--force-env` | 从 `.env.example` 重新生成 `.env`（自动备份旧文件） |
| `--dry-run` | 只打印将执行的命令，不做任何变更 |
| `-y, --yes` | 非交互确认 |
| `--down` | 停止容器（保留数据卷） |
| `--uninstall` | 停止并删除数据卷 |
| `--purge` | 配合 `--uninstall`，同时删除本地 `data/`、`logs/` |
| `--version` | 打印安装器版本 |
| `--help` | 查看用法 |

退出码：`0` 成功 · `1` 运行时失败 · `2` 参数错误 · `3` 预检失败（依赖/端口/磁盘）。

含 PostgreSQL、迁移、Controller、Agent。Console 容器需 UI 构建链；**可不装 Console**，见 A′。

分步执行（等价）：

```bash
cp .env.example .env
bash scripts/up.sh
bash ../scripts/smoke-test.sh
```

## 方式 A′ — Standalone（不依赖 Nerve-ui）

```bash
docker compose -f docker-compose.yml -f docker-compose.standalone.yml up -d --build
bash ../scripts/smoke-test.sh
```

## 方式 B — 预构建 OCI 镜像

```bash
docker pull ghcr.io/ahaiii/waf:1.0.0
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
cd nerve-waf
export DATABASE_URL=postgres://postgres:nerve_waf_dev@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh
make build
bash scripts/start-waf.sh
bash scripts/smoke-test.sh
```

## 方式 E — 无 PostgreSQL（文件存储）

Controller 内置文件存储后端，适合单机试用与离线演示，**不需要数据库**：

```bash
cd nerve-waf
cargo build -p nerve-waf-controller
NERVE_AUTH_ENABLED=0 ./target/debug/nerve-waf-controller --port 9000 --data-dir ./data
```

不传 `--database-url` / 不设 `DATABASE_URL` 时自动落到 `--data-dir` 下的文件存储；启动日志会打印
`config_backend="file" event_backend="file" database="file"`。**仅适用于单实例**，多副本请用 PostgreSQL。

## 安装后验证

一键安装会自动跑冒烟测试。手动复核：

```bash
bash scripts/smoke-test.sh --controller http://127.0.0.1:9000
```

| 检查 | 命令 | 期望 |
|------|------|------|
| 健康 | `curl -fsS http://127.0.0.1:9000/api/health` | `{"ok":true}` |
| 存储 | `curl -fsS http://127.0.0.1:9000/api/db/status` | 含 `configBackend` / `event_count` |
| 站点 | `curl -fsS http://127.0.0.1:9000/api/sites` | `sites` 数组 |
| 插件 | `curl -fsS http://127.0.0.1:9000/api/plugins` | `plugins` 数组 |

> 若本机配置了 `HTTP_PROXY`，访问 127.0.0.1 需加 `curl --noproxy '*'`。`smoke-test.sh` 已默认绕过代理。

### ⚠ 关键一步：站点必须绑定防护策略才会拦截

新装的站点默认**没有绑定策略**，此时所有请求都会 `allow`——这不是故障，是默认配置。
绑定内置策略后检测才会生效：

```bash
# 1) 查看可用模板（tpl-relaxed / tpl-standard / tpl-strict）与站点 id
curl -sS http://127.0.0.1:9000/api/protection-strategies
curl -sS http://127.0.0.1:9000/api/sites

# 2) 把「标准」策略应用到站点
curl -sS -X POST http://127.0.0.1:9000/api/protection-strategies/tpl-standard/apply \
  -H 'Content-Type: application/json' \
  -d '{"target":"app","site_id":"<你的 site_id>"}'

# 3) 重载规则
curl -sS -X POST http://127.0.0.1:9000/api/global/custom-rules/reload

# 4) 验证：正常请求放行，恶意样本拦截
curl -sS -X POST http://127.0.0.1:9000/api/inspect -H 'Content-Type: application/json' \
  -d '{"app_id":"<你的 site_id>","method":"GET","host":"a.example.com","uri":"/search?q=sqlmap","headers":"Accept: */*\r\n","body":"","src_ip":"198.51.100.66","user_agent":"sqlmap/1.7"}'
# 期望："action":"block"、"blocked":true，并返回 eventId
```

完成后再跑 `smoke-test.sh`，「检测拦截（恶意样本）」一项会由跳过变为通过。

## 升级

```bash
cd deploy
git pull                       # 或解压新版本
bash install.sh --no-build     # 复用现有镜像重启；需要新镜像时去掉 --no-build
bash ../scripts/smoke-test.sh
```

数据在具名卷（`postgres-data`）中，升级不删除。跨版本迁移：

```bash
export DATABASE_URL=postgres://...
bash scripts/apply-migrations.sh
```

## 卸载

```bash
cd deploy
bash install.sh --uninstall          # 停容器 + 删数据卷（交互确认）
bash install.sh --uninstall --purge  # 另外删除本地 data/ 与 logs/
```

## 故障排查

| 现象 | 原因 | 处理 |
|------|------|------|
| `端口被占用` | 9000/9080 已被其他进程使用 | `bash install.sh --port 9100`；查占用 `netstat -ano \| findstr :9000` |
| `Docker 守护进程不可用` | Docker 未启动 / 用户不在 docker 组 | 启动 Docker，或 `sudo usermod -aG docker $USER` 后重新登录 |
| `检测全部 allow` | 站点未绑定防护策略 | 见上文「关键一步」，绑定 `tpl-standard` 后 reload |
| 健康检查超时 | 镜像构建慢 / 数据库未就绪 | `docker compose logs --tail=100`；首次构建属正常 |
| `curl` 返回 502 但服务正常 | 系统级 `HTTP_PROXY` 拦截了 localhost | 加 `--noproxy '*'` |
| `/api/nerve/status` 返回 502 | Nerve 观察平台未部署（可选组件） | 忽略，或配置 `NERVE_URL` |

## 安装后

1. [getting-started.md](./getting-started.md)  
2. [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)（生产）  
3. [configuration.md](./configuration.md)（调参）
