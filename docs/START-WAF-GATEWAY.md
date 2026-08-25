# Starting Nerve WAF and Nerve Gateway separately

> **Note:** Optional sibling integration — **not required** to run Nerve WAF.  
> Product boundaries: [concepts/boundaries.md](./concepts/boundaries.md) · WAF-only install: [installation.md](./installation.md).

The two products are **independent directories + independent processes**:

| Product | Directory | Process | Port | Start |
|---------|-----------|---------|------|-------|
| Nerve WAF | `Nerve-waf/Nerve-waf` | `nerve-waf-controller` (+ Console) | Controller `:9000` | `scripts/start-waf.sh` or cargo |
| Nerve Gateway | `Nerve-gateway` | Gateway dataplane | often `:9080` | `scripts/start.sh` |

## Dependency

```text
Gateway ──HTTP──▶ WAF Control API
                    ├── WAF detection / policy / events
                    └── Gateway config (routes/upstreams/dataplane)
```

Gateway **requires** the WAF Control API first; WAF alone does not need Gateway.

## Minimal start (WSL / Linux)

```bash
# Terminal 1 — WAF
cd Nerve-waf/Nerve-waf && ./scripts/start-waf.sh

# Terminal 2 — Console (optional)
cd apps/console && npm run dev

# Terminal 3 — Gateway
cd Nerve-gateway && ./scripts/start.sh
```

## WAF only

```bash
cd Nerve-waf/Nerve-waf && ./scripts/start-waf.sh
```

## Gateway only (still needs a running WAF API)

```bash
cd Nerve-waf && ./scripts/start-waf.sh    # terminal 1
cd Nerve-gateway && ./scripts/start.sh    # terminal 2
```

## Directory layout

```text
<workspace-root>/
├── Nerve/           # Agent L3/L4 (optional)
├── Nerve-waf/       # WAF suite
└── Nerve-gateway/   # Gateway dataplane + plugins/
```

## Environment variables

| Variable | Default | Used by |
|----------|---------|---------|
| `DATABASE_URL` | (required) | WAF API |
| `NERVE_API_KEY` | — | WAF / clients |
| `UPSTREAM_URL` | `:8080` | Gateway default backend |
| `NERVE_WAF_CONTROLLER` | `:9000` | Gateway → WAF controller |
