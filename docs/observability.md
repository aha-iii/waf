# Observability

## Health endpoints

| 组件 | URL | K8s probe |
|------|-----|-----------|
| Controller | `GET /api/health` | Helm liveness/readiness |
| Agent | `GET /health` | Helm liveness/readiness |

Post-install: `bash scripts/smoke-test.sh`

## Logs

Configured in `config.yml` → `logging` ([configuration.md](./configuration.md)):

| File | Content |
|------|---------|
| `access.log` | HTTP access |
| `audit.log` | Admin actions |
| `error.log` | Errors |
| `server.log` | Process log |

Container path: `/var/lib/nerve-waf/logs` · Env: `RUST_LOG=info|debug`

```bash
# Kubernetes
kubectl -n nerve-waf logs deploy/nerve-waf-controller -f
kubectl -n nerve-waf logs deploy/nerve-waf-agent -f
# daemonset: kubectl -n nerve-waf logs daemonset/nerve-waf-agent -f
```

## Agent sync visibility

Successful load: log line `dataplane loaded` with `generation`, `routes`, `upstreams`.  
Failures: `watch failed`, `dataplane refresh` — see [architecture-overview.md](./architecture-overview.md) §5.

## Metrics

| Status | Detail |
|--------|--------|
| **Current** | No `/metrics` Prometheus exposition |
| **Interim** | Blackbox probe on `/api/health` and Agent `/health` |
| **Helm** | Optional `metrics.serviceMonitor.enabled` (availability scrape via Prometheus Operator) |
| **Roadmap** | Request/block/latency counters — tracked in CHANGELOG |

## Dashboards

Until native metrics ship, recommend:

- Log aggregation on JSON `access.log` / `audit.log`
- Alert on Controller health + Agent health + PG connectivity
- Alert on Agent log rate of `dataplane refresh` warnings

## Runbooks

[runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)
