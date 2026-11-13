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

## Optional integrations

| Feature flag | Dependency | How to get it | Purpose |
|--------------|------------|---------------|---------|
| `alert` | HTTP dispatch to Nerve-alert (`--features alert`, env `NERVE_ALERT_URL`) | 无 crate 依赖，纯 HTTP | 告警分发 |
| `expert` | `nerve-antibody` | Cargo 自动拉取 `github.com/aha-iii/antibody` | Expert 检测阶段 |

`expert` 是跨仓库依赖，声明为 **git 依赖**，无需手工排列目录：

```toml
nerve-antibody = { git = "https://github.com/aha-iii/antibody", branch = "main", optional = true }
```

本地联调两个仓库时，放一个不入库的 `.cargo/config.toml` 指向同级工作副本即可：

```toml
[patch."https://github.com/aha-iii/antibody"]
nerve-antibody = { path = "../Nerve-antibody/crates/integration/nerve-antibody" }
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
