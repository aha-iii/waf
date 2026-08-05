# Nerve WAF 插件目录

本目录存放 **可热加载** 的外置插件。控制面启动或 `POST /api/plugins/reload` / `POST /api/sync` 时自动扫描。

## 目录结构

```
plugins/
├── README.md          ← 本文件
├── l3/                ← L3 网络策略（Rust 内置 + 路由配置）
└── l7/                ← L7 HTTP 检测（JSON 规则包 + Lua 脚本）
```

## 插件类型

| type | 语言 | 热更新 | 用途 |
|------|------|--------|------|
| `rust` | Rust | 需编译进 `nerve-waf-plugins` | L3 deny/allow/baseline（核心） |
| `json` | JSON 规则 | ✅ 改文件 + reload | OWASP 规则包 |
| `lua` | Lua 5.4 | ✅ 改脚本 + reload | 自定义检测逻辑 |

## 环境变量

- `NERVE_WAF_PLUGINS`：插件根目录（默认 `Nerve-waf/plugins`）

## 生效流程

```
1. 编辑 plugins/l7/... 或 data/routes.json / data/modules.json
2. POST http://localhost:8780/api/plugins/reload   # 仅重载 L7 规则/Lua
   或 POST http://localhost:8780/api/sync          # L3 下发 Agent + 重载 L7
3. POST /api/inspect 或 Gateway 下一请求使用新规则
```

## API

- `GET /api/plugins` — 列出 Rust + JSON + Lua 插件
- `POST /api/plugins/reload` — 热重载 plugins/ 目录

## 新增 JSON 规则包

```bash
mkdir -p plugins/l7/my-pack/rules
# 编写 plugin.json + rules/*.json
curl -X POST http://127.0.0.1:8780/api/plugins/reload
```

## 新增 Lua 插件

见 `plugins/l7/custom-header-check/` 示例。

## 新增 Rust L3 插件

见 `crates/engine/nerve-waf-plugins/README.md`，编译后出现在 `GET /api/plugins` 的 `type=rust` 列表。
