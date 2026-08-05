# nerve-waf-coraza

**进程内嵌 [OWASP Coraza](https://coraza.io/) + CRS v4**（Go）。  
用于对齐 ModSecurity/Coraza 全集语义；Rust `nerve-waf-l7` 仍负责自定义/JSON 子集与业务模块。

## 为何是 Go 进程

Coraza 官方实现是 Go，没有可用的生产级 Rust 移植。Nerve 的「嵌入」= **本二进制内嵌 Coraza 库 + CRS**，由 WAF API / Gateway 经 HTTP 调用（默认 `127.0.0.1:8791`）。

## 启动

```bash
# WSL
cd Nerve-waf/coraza-engine
go run ./cmd/nerve-waf-coraza
# 或
./scripts/start-coraza.sh
```

环境变量：

| 变量 | 默认 | 说明 |
|------|------|------|
| `NERVE_CORAZA_LISTEN` | `127.0.0.1:8791` | 监听地址 |

## API

- `GET /healthz`
- `POST /api/inspect` — body 与 WAF inspect 字段对齐（method/host/uri/headers/body/src_ip/user_agent）

## Rust 侧

设 `NERVE_WAF_CORAZA_URL=http://127.0.0.1:8791`（或 `NERVE_WAF_CRS_ENGINE=coraza`）后，InspectPipeline / edge 的 **crs 阶段**走 Coraza。
