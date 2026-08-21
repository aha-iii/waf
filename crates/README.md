# Nerve WAF Crate 分层

产品名 **Nerve WAF**；crate 名 **`nerve-waf-*`**（见仓库根 [docs/NAMING.md](../docs/NAMING.md)）。
顶层按产品边界分组，仅目录归类。

```text
crates/
├── contract/                  【契约】跨控制面/数据面类型
│   ├── nerve-waf-sdk
│   └── nerve-waf-dataplane    WafEdgeSnapshot
│
├── engine/                    【检测引擎】无 IO、无 Store
│   ├── nerve-waf-core
│   ├── nerve-waf-inspect
│   ├── nerve-waf-l7
│   ├── nerve-waf-plugin-host
│   ├── nerve-waf-plugins
│   ├── nerve-waf-engine
│   ├── nerve-waf-normalize
│   ├── nerve-waf-semantic
│   ├── nerve-waf-coraza
│   ├── nerve-waf-wasm
│   └── nerve-waf-libinjection
│
├── modules/                   【流水线模块】overlay / 策略插件
│   ├── nerve-waf-cc
│   ├── nerve-waf-geo
│   ├── nerve-waf-bot
│   ├── nerve-waf-behavior
│   ├── nerve-waf-whitelist
│   ├── nerve-waf-intel
│   ├── nerve-waf-api-sec
│   ├── nerve-waf-challenge
│   └── nerve-waf-triage
│
├── controlplane/              【控制面】API + Store + 运维
│   ├── nerve-waf-api          src/controlplane + src/dataplane
│   ├── nerve-waf-store
│   ├── nerve-waf-auth
│   ├── nerve-waf-rbac
│   ├── nerve-waf-license
│   ├── nerve-waf-rules-ops
│   ├── nerve-waf-crs-import
│   └── …
│
└── dataplane/                 【数据面进程 / 内嵌运行时】
    ├── nerve-waf-edge
    └── nerve-waf-embed
```

Workspace 路径见仓库根 `Cargo.toml`。同级依赖见 [docs/DEPENDENCIES.md](../docs/DEPENDENCIES.md)。

## 测试目录约定

生产代码与测试 **物理分离**：每个 crate 在 `src/test/` 下放 `test_*.rs`，`lib.rs` 末尾仅保留：

```rust
#[cfg(test)]
mod test;
```

```text
crates/<layer>/<crate>/
├── src/
│   ├── lib.rs              # 生产逻辑，不含 #[test]
│   └── test/
│       ├── mod.rs
│       └── test_<topic>.rs
```

- 运行：`cargo test -p <crate-name>`
- **参考实现**：`nerve-waf-dataplane`、`nerve-waf-embed`、`nerve-waf-l7`、`nerve-waf-core`
- 旧 `#[cfg(test)] mod tests` 内联块逐步迁出（贡献新测试时请直接用 `src/test/`）
