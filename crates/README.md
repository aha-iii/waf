# Nerve WAF crate layout

Product name **Nerve WAF**; crate names **`nerve-waf-*`** (see repo-root [docs/NAMING.md](../docs/NAMING.md)).
Top-level folders group by product boundary only.

```text
crates/
├── contract/                  [contract] cross control/dataplane types
├── engine/                    [detection engine] no IO, no Store
├── modules/                   [pipeline modules] overlay / policy plugins
├── controlplane/              [control plane] API + Store + ops
└── dataplane/                 [dataplane process / embed runtime]
```

Workspace paths: repo-root `Cargo.toml`. Sibling deps: [docs/DEPENDENCIES.md](../docs/DEPENDENCIES.md).

## Test layout

Keep production code and tests **physically separated**: each crate puts `test_*.rs` under `src/test/`; crate root (`lib.rs` / `main.rs`) ends with only:

```rust
#[cfg(test)]
mod test;
```

```text
src/
├── lib.rs              # production logic, no #[test]
└── test/
    ├── mod.rs
    └── test_*.rs
```

- Run: `cargo test -p <crate-name>`
- Do **not** put `#[cfg(test)] mod tests { … }` inside production modules; add new tests under `src/test/` only.
