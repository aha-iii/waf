# Sibling dependencies (why you cannot “clone only this directory”)

The Nerve WAF workspace depends on other ecosystem repos via **Cargo path**. Current resolution (relative to this repo root `Nerve-waf/Nerve-waf/`):

| Dependency | Cargo coord | Expected path on disk | Purpose |
|------------|-------------|------------------------|---------|
| Antibody integration | `nerve-antibody` | `../Nerve-antibody/crates/integration/nerve-antibody` | inspect `expert` stage |
| Antibody metrics | `nerve-antibody-metrics` | `../Nerve-antibody/crates/engine/facade/nerve-antibody-metrics` | optional tracing |
| Alert client/sdk | `nerve-alert-client` / `nerve-alert-sdk` | `../../Nerve-alert/crates/...` | alert dispatch / Console proxy |

Matching monorepo layout:

```text
<workspace-root>/                 # e.g. waf/
├── Nerve-alert/                  # alerts (needed for full API build)
└── Nerve-waf/
    ├── Nerve-waf/                # this repo
    └── Nerve-antibody/           # expert engine + rule packs
```

## Minimal runnable combinations

| Goal | Needs |
|------|-------|
| Build L7 / core / modules only (no expert, no alert API) | Still keep Antibody: `nerve-waf-inspect` **hard-depends** on the `nerve-antibody` path today |
| Full WAF API + Console | Nerve-waf + Antibody + Alert + PostgreSQL |
| Edge dataplane | Nerve-waf (wasm CRS) + config; expert needs Antibody packs |

> Even when a path dependency is marked optional, Cargo still requires the directory to exist at resolve time. True single-repo crates.io publish needs versioned or git deps for client/sdk/antibody later; until then, clone siblings side-by-side.

## Check and fetch

Place sibling repos yourself (table above). Release scripts: `scripts/README.md` (`deploy-test` / `deploy-prod`).

Local development: see repo-root [README.md](../README.md) and [deploy/START.md](../deploy/START.md).
