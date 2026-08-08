# 两步启动（在 Nerve-waf/deploy 目录）

## 1. 构建或加载镜像

**方式 A：已有 tar（推荐）**

```powershell
docker load -i ..\dist\nerve-waf-api-1.0.0.tar
```

**方式 B：本地重新打包**

```powershell
# 全量容器内编译（慢，上下文为 waf/ 根目录）
powershell -File scripts\build-image.ps1

# 或 WSL 已 cargo build --release 后：
# $env:USE_PREBUILT = "true"; powershell -File scripts\build-image.ps1
```

镜像内已包含：

- Console 静态页（构建时 `VITE_CONSOLE_TIME_ZONE=Asia/Shanghai`）
- Native baseline 规则包 → `/opt/nerve/native/baseline`
- 时区 `TZ=Asia/Shanghai`

## 2. 配置（按需改这三处）

### `.env`（环境变量，compose 自动加载）

```ini
PORT=8781
TZ=Asia/Shanghai

NERVE_AUTH_ENABLED=0          # 测试关鉴权；生产改 1 并设 NERVE_API_KEY
NERVE_API_KEY=dev-nerve-key

DATABASE_URL=postgres://postgres:p@ssw0rd@192.168.10.16:5432/nerve

NERVE_EXPERT_ENGINE=on
NERVE_EXPERT_PACK=/opt/nerve/native/baseline
NERVE_NATIVE_DEV_PACK=1       # 明文 pack；生产 sealed 包改 0

RUST_LOG=info
```

### `config/config.yml`（WAF 主配置，挂载进容器）

```yaml
waf:
  api:
    port: 8781
  data_dir: "/var/lib/nerve-waf/data"
  plugins_dir: "/opt/nerve-waf/plugins"
  nerve_url: "http://host.docker.internal:8766"
  database:
    url: "postgres://postgres:p@ssw0rd@192.168.10.16:5432/nerve"

logging:
  level: info
  access_log: "/var/lib/nerve-waf/logs/access.log"
  # ...
```

### `docker-compose.yml` / `docker-compose.yaml`

**两个服务**（同一镜像 `nerve/nerve-waf-api`）：

| 服务 | 端口 | 作用 |
|------|------|------|
| `nerve-waf-api` | `${PORT:-8781}` | API + **Console Dashboard**（管理面） |
| `nerve-waf-edge` | `${EDGE_PORT:-8080}` | 业务流量回源 + WAF 检测（数据面） |

开发脚本 `start-waf-edge-stack.sh` 会起 **三个进程**（API + Edge + Vite Console）；Docker 镜像内 Console 已打进 API 容器，**不需要第三个容器**。

业务域名 → **8080**；管理 Console → **8781**（建议内网）。

## 3. 启动

```powershell
docker compose up -d
```

## 验证

```powershell
curl http://127.0.0.1:8781/api/health
curl http://127.0.0.1:8781/api/expert/health
curl http://127.0.0.1:8080/health
# Console（管理面，勿将业务域名指到 8781）
start http://127.0.0.1:8781/
```

业务域名应解析到 **Edge 8080**（或 LB 443 → 8080），例如：

```bash
curl -H "Host: test-mars.rektec.cn" http://127.0.0.1:8080/
```

## 停止 / 删除

```powershell
docker compose down
docker compose down -v   # 同时删数据卷
```

## 生产注意

| 项 | 测试默认 | 生产建议 |
|----|----------|----------|
| `NERVE_AUTH_ENABLED` | `0` | `1` + Secret 管理 `NERVE_API_KEY` |
| `NERVE_NATIVE_DEV_PACK` | `1`（明文 baseline） | 明文目录必须为 `1`；仅当 `NERVE_EXPERT_PACK` 指向 `.sealed` + License 时设 `0` |
| `DATABASE_URL` | 内网 PG | 生产库连接串 |
| Console 时区 | 构建参数固定东八区 | 改时区需重建镜像 `--build-arg VITE_CONSOLE_TIME_ZONE=...` |

Helm：`helm/nerve-waf/values.yaml` 中 `env.TZ`、`env.NERVE_EXPERT_*` 已对齐。
