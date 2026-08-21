# nerve-waf-coraza

**In-process [OWASP Coraza](https://coraza.io/) + CRS v4** (Go).  
Aligns with full ModSecurity/Coraza semantics; Rust `nerve-waf-l7` still owns custom/JSON subsets and business modules.

## Why a Go process

The official Coraza implementation is Go; there is no production-grade Rust port. Nerve “embedding” means **this binary embeds Coraza + CRS**, and WAF API / Gateway call it over HTTP (default `127.0.0.1:8791`).

## Start

```bash
# WSL
cd Nerve-waf/coraza-engine
go run ./cmd/nerve-waf-coraza
# or
./scripts/start-coraza.sh
```

Environment:

| Variable | Default | Description |
|----------|---------|-------------|
| `NERVE_CORAZA_LISTEN` | `127.0.0.1:8791` | Listen address |

## API

- `GET /healthz`
- `POST /api/inspect` — body aligned with WAF inspect fields (method/host/uri/headers/body/src_ip/user_agent)

## Rust side

Set `NERVE_WAF_CORAZA_URL=http://127.0.0.1:8791` (or `NERVE_WAF_CRS_ENGINE=coraza`) so InspectPipeline / edge **crs stage** uses Coraza.
