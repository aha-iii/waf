# Nerve WAF deploy (publishable examples)

Private prod/test host materials live on the maintainer machine under **`ops/`** (not in Git).

Use this directory after clone: copy configs and start:

| Path | Purpose |
|------|---------|
| [START.md](./START.md) | Step-by-step startup |
| `.env.example` | Copy to `.env` |
| `config/config.yml` | Sample WAF config |
| `docker-compose.yml` | API + Edge |
| `docker/` | Dockerfiles |
| `helm/` | Helm chart |

```bash
cp .env.example .env
# Edit DATABASE_URL and related settings
docker compose up -d
```
