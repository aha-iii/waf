# Sibling dependencies (optional integrations)

Nerve WAF builds and runs **standalone** by default (CRS/WASM inspect, control API, Edge).  
Optional features use sibling repos via Cargo path + feature flags.

## Default (standalone clone)

```bash
git clone <nerve-waf-repo> nerve-waf
cd nerve-waf
cargo build -p nerve-waf-controller --release
```

No `Nerve-antibody` or `Nerve-alert` directory required.

| Capability | Default build |
|------------|---------------|
| L7 inspect (WASM/Coraza, modules) | Yes |
| Control API + PostgreSQL | Yes |
| Edge dataplane | Yes |
| Expert engine (`nerve-antibody`) | No — enable `--features expert` |
| Alert dispatch (`nerve-alert`) | No — enable `--features alert` |

## Optional siblings

| Feature flag | Dependency | Expected path (from repo root) | Purpose |
|--------------|------------|---------------------------------------------|---------|
| `alert` | HTTP dispatch to Nerve-alert service (`--features alert`, env `NERVE_ALERT_URL`) | No separate `nerve-alert` crate required |
| `expert` | `nerve-antibody` | `../Nerve-antibody/`（与本仓库并列克隆） | First-party expert inspect stage |

Side-by-side sibling layout (all features):

```text
<parent>/
├── nerve-waf/            # this repo
└── Nerve-antibody/       # sibling clone (--features expert)
```

Build with all integrations:

```bash
cargo build -p nerve-waf-controller --release --features full
```

Environment toggles (runtime, even when compiled in):

| Variable | Effect |
|----------|--------|
| `NERVE_EXPERT_ENGINE=off` | Skip expert stage at runtime |
| `NERVE_ALERT_ENABLED=0` | Disable alert HTTP dispatch |

## Database

PostgreSQL schema is **not** applied by the API process. Use:

```bash
bash scripts/apply-migrations.sh
```

Docker Compose runs the same script via the `db-migrate` service.

## Console UI

`apps/console` depends on `@nerve/ui` (`Nerve-ui` in the wider monorepo). API/Edge docker image builds without Console static assets; add Nerve-ui to build the full Console bundle.
