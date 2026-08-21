# Nerve WAF

**Nerve WAF** 是 Nerve WAF 套件中的防护产品：L7 检测流水线、控制面 API、WAF Console、边缘数据面。  
Rust crate / 二进制仍使用稳定前缀 **`nerve-waf-*`**（见 [docs/NAMING.md](./docs/NAMING.md)）。

| 组件 | 默认 | 说明 |
|------|------|------|
| Control API | `:8781` | `nerve-waf-api`（L7 inspect、站点、事件、WAF-L3 sync） |
| Console | `:5174` | `apps/console-react`（Vite） |
| Nerve Agent（可选） | `:8766` | 生态仓 `Nerve/`，WAF-L3 → eBPF |
| Nerve-alert（可选完整告警） | `:8790` | 同级仓 `Nerve-alert/` |
| Nerve-control（拓扑等） | `:8780` | **不属于**本产品长期职责 |

同级依赖说明：[docs/DEPENDENCIES.md](./docs/DEPENDENCIES.md) · 架构：[docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) · 贡献：[CONTRIBUTING.md](./CONTRIBUTING.md) · 安全：[SECURITY.md](./SECURITY.md)

## 快速开始

```bash
# 0) 同级仓检查（Antibody / Alert）
bash scripts/check-sibling-deps.sh

# 1) 环境
cp .env.example .env

# 2) API
bash scripts/start-waf.sh          # 或 start-api.sh → :8781

# 3) Console
bash scripts/start-waf-console.sh  # console-react；亦可用 start-console.sh（已指向 console-react）
```

```bash
cargo build --release -p nerve-waf-api
NERVE_WAF_DATA=./data cargo run -p nerve-waf-api --release
```

节点级端口与数据库见根目录 `config.yml`；环境变量覆盖见 `.env.example`。

## Workspace 分层

```text
crates/
├── contract/       # nerve-waf-sdk、nerve-waf-dataplane
├── engine/         # core / inspect / l7 / wasm / normalize …
├── modules/        # cc / geo / bot / behavior / whitelist …
├── controlplane/   # api / store / auth / rbac …
└── dataplane/      # edge / embed
apps/console-react/ # WAF Console
plugins/            # L7 JSON/Lua、L3 说明
scripts/            # 启动、CI、冒烟（见 scripts/README.md）
vendor/coreruleset/ # OWASP CRS 快照（见 vendor/README.md）
```

检测流水线（`POST /api/inspect`）：

```text
PreAccess (CC / Geo / Bot / …)
  → expert（Antibody，需同级仓）
  → core（L7 / libinjection / semantic / WASM CRS）
  → custom
→ InspectVerdict
```

## 主要 API

- `POST /api/inspect` — 完整检测（Gateway / APISIX 旁路）
- `GET|PUT /api/modules`、`/api/sites`、`/api/events`
- `POST /api/compile`、`POST /api/sync` — WAF-L3 → Agent
- `GET|PUT /api/alerts/*` — 代理到 Nerve-alert（需同级仓）

## 许可证

[MIT](./LICENSE)
