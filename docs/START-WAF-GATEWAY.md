# Nerve WAF 与 Nerve Gateway 分开启动

两个产品是**独立目录 + 独立进程**：

| 产品 | 目录 | 进程 | 端口 | 启动 |
|------|------|------|------|------|
| **Nerve WAF** | `Nerve-waf/` | `nerve-waf-api` | **8780** | `./scripts/start-waf.sh` |
| **Nerve Gateway** | `Nerve-gateway/` | `nerve-gateway` | **8443** | `./scripts/start.sh` |
| **Console** | `Nerve-waf/apps/console` | Vite | **5174** | `Nerve-waf/scripts/start-console.sh` |

## 依赖关系

```
Console :5174  ──proxy──►  Nerve-waf Control API :8780  ◄──poll──  Nerve-gateway :8443
                                │
                                ├── WAF 检测 / 策略 / 事件
                                └── Gateway 配置 (routes/upstreams/dataplane)
```

Gateway **必须先有** WAF Control API；纯 WAF 可不启 Gateway。

## 最小启动（WSL / Linux）

```bash
# 终端 1 — WAF
cd Nerve-waf
cp .env.example .env
chmod +x scripts/*.sh
./scripts/start-waf.sh

# 终端 2 — Console（可选）
./scripts/start-console.sh

# 终端 3 — Gateway
cd ../Nerve-gateway
cp .env.example .env
chmod +x scripts/start.sh
./scripts/start.sh
```

## 只跑 WAF

```bash
cd Nerve-waf && ./scripts/start-waf.sh
curl -X POST http://127.0.0.1:8780/api/inspect \
  -H 'Content-Type: application/json' \
  -d '{"method":"GET","host":"t","uri":"/?id=1 union","headers":"","body":"","src_ip":"1.2.3.4","user_agent":"curl"}'
```

## 只跑 Gateway

```bash
cd Nerve-waf && ./scripts/start-waf.sh    # 终端 1
cd Nerve-gateway && ./scripts/start.sh    # 终端 2
curl http://127.0.0.1:8443/healthz
```

## 目录结构

```
waf/
├── Nerve/           # Agent L3/L4（可选）
├── Nerve-waf/       # WAF Control API + Console + plugins/l7,l3
└── Nerve-gateway/   # Gateway 数据面 + plugins/
```

## 环境变量

| 变量 | 默认 | 用于 |
|------|------|------|
| `PORT` | 8780 | WAF API（Nerve-waf/.env） |
| `DATABASE_URL` | — | PostgreSQL |
| `GATEWAY_PORT` | 8443 | Gateway（Nerve-gateway/.env） |
| `UPSTREAM_URL` | :8080 | Gateway 默认后端 |
| `NERVE_WAF_API` | :8780 | Gateway → WAF API 地址 |
