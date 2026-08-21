# Contributing

Thanks for contributing. The product name is **Nerve WAF**; Rust crates keep the stable prefix **`nerve-waf-*`** (see [docs/NAMING.md](./docs/NAMING.md)).

## Before you start

1. Read [README.md](./README.md) and [docs/DEPENDENCIES.md](./docs/DEPENDENCIES.md)
2. Place sibling repos per DEPENDENCIES (Antibody / Alert)
3. Rust: see `rust-toolchain.toml`; Node 20+ (Console)

```bash
cp .env.example .env          # edit as needed; do not commit .env
```

## Quality bar (required for PRs)

| Check | Command / convention |
|-------|----------------------|
| Build | `cargo build -p nerve-waf-api` |
| Related unit tests | `cargo test -p <changed-crate>` |
| Format | `cargo fmt --all -- --check` |
| Clippy (touched crates) | `cargo clippy -p <crate> -- -D warnings` (preferred) |

CI entrypoint: monorepo-root `.github/workflows/waf-ai-ci.yml`. Locally, at least run tests for crates you changed.

## Test layout

Keep production code and tests **physically separated**:

```text
src/
├── lib.rs                 # no #[test]
└── test/
    ├── mod.rs
    └── test_*.rs
```

End of `lib.rs` should only have:

```rust
#[cfg(test)]
mod test;
```

- Put new tests in `src/test/test_*.rs`; **do not** add `#[cfg(test)] mod tests` inside business files
- Migrate old inline tests gradually; see `nerve-waf-dataplane`, `nerve-waf-l7`

## Paths and scripts

| Path | For contributors? |
|------|-------------------|
| `crates/`, `apps/console-react/` | Yes — main work areas |
| `ops/` (local release/deploy scripts) | No — **not on GitLab** (see `ops/README.md`) |
| `vendor/coreruleset` | Upstream CRS snapshot; see `vendor/README.md` |
| `dist/`, `data/`, `logs/` | No — local artifacts; do not commit |

## PR tips

1. Small commits; one concern per PR
2. Update README or `docs/` for user-visible behavior changes
3. For security-sensitive changes, also read [SECURITY.md](./SECURITY.md)
4. Do not commit secrets, private keys, or real production host inventories

## Code of conduct

Stay professional and on-topic. Harassment and contributions that enable abuse will be rejected.
