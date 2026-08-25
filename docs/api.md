# HTTP API reference

Machine-readable spec: [openapi.yaml](./openapi.yaml)（可导入 Postman / codegen）。

Base URL：`http://<controller-host>:9000`

## Authentication

| Header | When |
|--------|------|
| `X-API-Key: <NERVE_API_KEY>` | `NERVE_AUTH_ENABLED=1`（生产） |

Public (no key): `GET /api/health`, auth login endpoints.

**`/api/inspect` always requires auth** when `NERVE_AUTH_ENABLED=1`.

## Control plane

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Liveness / readiness |
| GET | `/api/auth/status` | Auth configuration |
| POST | `/api/auth/login` | Console local login |
| POST | `/api/inspect` | Bypass L7 inspect (JSON body) |
| GET/PUT | `/api/sites` | Protected sites |
| GET/PUT | `/api/modules` | Module toggles |
| GET | `/api/events` | Security events |

## Agent sync (Controller side)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/waf/edge/dataplane` | Full snapshot |
| GET | `/api/waf/edge/watch` | Long-poll (`since`, `timeout_sec`) |
| POST | `/api/waf/edge/ack` | Body `{ "generation": N }` |
| GET | `/api/waf/edge/status` | Edge registration status |

Agent 行为见 [architecture-overview.md](./architecture-overview.md) §4。

## Agent dataplane

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Agent process health (port 9080) |
| * | `/*` | Routed proxy + inspect |

## Examples

```bash
curl -fsS http://127.0.0.1:9000/api/health

curl -fsS -X POST http://127.0.0.1:9000/api/inspect \
  -H "Content-Type: application/json" \
  -H "X-API-Key: $NERVE_API_KEY" \
  -d '{"method":"GET","uri":"/?id=1+UNION+SELECT","headers":{}}'
```

## Errors

| HTTP | Meaning |
|------|---------|
| 401 | Missing/invalid API key |
| 403 | Policy block (inspect) |
| 503 | Dependency down (e.g. database) |

## Versioning

Current: unversioned `/api/*`. Breaking changes tracked in [CHANGELOG.md](../CHANGELOG.md).
