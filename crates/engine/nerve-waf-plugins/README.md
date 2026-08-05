# 内置 L3 策略插件（Rust）

通过 `WafPlugin` trait 注册，在 `routes.json` 中按名称引用。

| 插件 | 配置示例 |
|------|----------|
| `ip-blacklist` | `{ "ips": ["1.2.3.4"] }` |
| `ip-whitelist` | `{ "ips": ["10.0.0.1"] }` |
| `port-block` | `{ "ports": [4444], "deny_dst": true }` |
| `baseline-allow` | `{ "pod": "frontend", "ip": "8.8.8.8" }` |

## 新增插件

1. 新建 `src/my_plugin.rs`，实现 `WafPlugin`
2. 在 `registry.rs` 的 `plugin_registry()` 注册
3. `cargo build -p nerve-waf-api`
4. 在 `data/routes.json` 引用插件名
5. `POST /api/sync` 下发 Agent

L7 JSON/Lua 插件见仓库根目录 `plugins/l7/`，无需改本 crate。
