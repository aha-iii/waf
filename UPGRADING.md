# Upgrading Nerve WAF

## Version policy

| Artifact | Source of truth |
|----------|-----------------|
| Application semver | `VERSION` / workspace `Cargo.toml` |
| Helm Chart | `deploy/helm/nerve-waf/Chart.yaml` (`version` + `appVersion`) |
| Docker / Compose `TAG` | `deploy/.env.example` — match application semver |

Release tags: `nerve-waf-v1.0.0`, `nerve-waf-v1.1.0`, …

## 0.1.0 → 1.0.0 (Helm)

Helm values are **component-scoped** (Cilium/K8s style):

| Old (0.1.0) | New (1.0.0) |
|-------------|-------------|
| `replicaCount` | `controller.replicaCount` |
| `service.port` | `controller.service.port` |
| `env.*` | `controller.env.*` + top-level `database.url` |
| _(none)_ | `agent.enabled`, `agent.mode`, `agent.service.port` |

Example migration:

```yaml
# before
replicaCount: 2
service:
  port: 9000
env:
  DATABASE_URL: postgres://...

# after
controller:
  replicaCount: 2
  service:
    port: 9000
agent:
  enabled: true
database:
  url: postgres://...
```

Upgrade:

```bash
helm upgrade nerve-waf ./deploy/helm/nerve-waf -n nerve-waf -f your-values.yaml
kubectl -n nerve-waf rollout status deploy/nerve-waf-controller
kubectl -n nerve-waf rollout status deploy/nerve-waf-agent
```

## Database migrations

Always run migrations **before** rolling out new Controller pods:

```bash
export DATABASE_URL=postgres://...
bash scripts/apply-migrations.sh
```

Compose stacks run `db-migrate` automatically on `up`.

## Docker Compose

```bash
cd deploy
# set TAG in .env to target version
docker compose pull   # when using published images
docker compose up -d --build
bash ../scripts/smoke-test.sh
```

## Rollback

- **Helm**: `helm rollback nerve-waf <revision> -n nerve-waf`
- **Compose**: set previous `TAG` in `.env`, `docker compose up -d`
- **Config**: restore `deploy/config/config.yml` from backup / Git

See [docs/runbooks/PRODUCTION.md](./docs/runbooks/PRODUCTION.md).
