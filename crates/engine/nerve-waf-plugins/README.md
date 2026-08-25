# Built-in L3 policy plugins (Rust)

Registered via the `WafPlugin` trait and referenced by name in `routes.json`.

| Plugin | Config example |
|--------|----------------|
| `ip-blacklist` | `{ "ips": ["1.2.3.4"] }` |
| `ip-whitelist` | `{ "ips": ["10.0.0.1"] }` |
| `port-block` | `{ "ports": [4444], "deny_dst": true }` |
| `baseline-allow` | `{ "pod": "frontend", "ip": "8.8.8.8" }` |

## Add a plugin

1. Create `src/my_plugin.rs` implementing `WafPlugin`
2. Register it in `plugin_registry()` in `registry.rs`
3. `cargo build -p nerve-waf-controller`
4. Reference the plugin name in `data/routes.json`
5. `POST /api/sync` to push to the Agent

L7 JSON/Lua plugins live under repo-root `plugins/l7/`; no changes to this crate required.
