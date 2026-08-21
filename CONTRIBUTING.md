# Contributing to Nerve WAF

感谢贡献。本仓库产品名为 **Nerve WAF**；Rust crate 仍使用稳定前缀 **`nerve-waf-*`**（见 [docs/NAMING.md](./docs/NAMING.md)）。

## 开发前准备

1. 阅读 [README.md](./README.md) 与 [docs/DEPENDENCIES.md](./docs/DEPENDENCIES.md)
2. 按 DEPENDENCIES 放好同级仓（Antibody / Alert），或运行 `bash scripts/check-sibling-deps.sh`
3. Rust：见 `rust-toolchain.toml`；Node 20+（Console）

```bash
cp .env.example .env          # 按需修改，勿提交 .env
cargo build -p nerve-waf-api
cargo test -p nerve-waf-core -p nerve-waf-l7 -p nerve-waf-dataplane
```

## 质量门槛（PR 必过）

| 检查 | 命令 / 约定 |
|------|-------------|
| 编译 | `cargo build -p nerve-waf-api` |
| 相关单测 | `cargo test -p <changed-crate>` |
| 格式 | `cargo fmt --all -- --check` |
| Clippy（改动 crate） | `cargo clippy -p <crate> -- -D warnings`（尽量） |

CI 入口参考仓库根（monorepo）`.github/workflows/waf-ai-ci.yml`；本地至少跑与改动相关的 crate 测试。

## 测试目录约定

生产代码与测试 **物理分离**：

```text
crates/<layer>/<crate>/src/
├── lib.rs                 # 不含 #[test]
└── test/
    ├── mod.rs
    └── test_<topic>.rs
```

`lib.rs` 末尾仅：

```rust
#[cfg(test)]
mod test;
```

- 新测试请写入 `src/test/test_*.rs`，**不要**在业务文件里新增 `#[cfg(test)] mod tests`
- 旧内联测试可逐步迁出；参考 `nerve-waf-dataplane`、`nerve-waf-l7`

## 目录与脚本

| 路径 | 是否面向贡献者 |
|------|----------------|
| `crates/`、`apps/console-react/` | ✅ 主战场 |
| `scripts/start-*.sh`、`download-*.sh`、`import-crs.sh`、`defense-smoke-*` | ✅ 开发/验证 |
| `scripts/diag-*`、`deploy-prod*`、`emergency-*` 等运维脚本 | ❌ **不入库**（见根 `.gitignore` / `scripts/README.md`） |
| `vendor/coreruleset` | 上游 CRS 快照，勿随手大改；见 `vendor/README.md` |
| `dist/`、`data/`、`logs/` | ❌ 本地产物，勿提交 |

## PR 建议

1. 小步提交；一次 PR 一件事
2. 用户可见行为变更请更新 README 或 `docs/`
3. 安全相关改动请同步看 [SECURITY.md](./SECURITY.md)
4. 不要提交密钥、证书私钥、真实生产主机清单

## 行为准则

保持专业、就事论事。骚扰与恶意利用相关贡献一律拒绝。
