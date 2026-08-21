# Quick start (Docker)

Work from `Nerve-waf/Nerve-waf/deploy`. Requires Docker and PostgreSQL (local or container).

## 1. Prepare config

```bash
cp .env.example .env
# Edit .env: at least DATABASE_URL and (for production) NERVE_API_KEY

# Also sync database.url in config/config.yml, or override with DATABASE_URL only
```

Placeholder conventions:

| Variable | Example |
|----------|---------|
| `DATABASE_URL` | `postgres://postgres:CHANGE_ME@127.0.0.1:5432/nerve` |
| `NERVE_API_KEY` | `change-me-api-key` |
| Console demo login (optional) | Backend bootstrap `admin` / `change-me`; frontend `?demo=1` prefills the same |

## 2. Images

```powershell
# Existing tar
docker load -i ..\dist\nerve-waf-api-1.0.0.tar

# Or build locally (scripts under ops/, maintainer machine only)
# powershell -File ..\ops\deploy-scripts\build-image.ps1
```

## 3. Start

```powershell
docker compose up -d
```

| Service | Default port | Role |
|---------|--------------|------|
| `nerve-waf-api` | `8781` | API + Console |
| `nerve-waf-edge` | `8080` | Traffic inspection |

Health checks:

- `http://127.0.0.1:8781/api/health`
- `http://127.0.0.1:8080/health`

## 4. Source development (no Docker)

```bash
cd ..   # Nerve-waf/Nerve-waf
cp .env.example .env
cargo run -p nerve-waf-api --release
cd apps/console-react && npm install && npm run dev
```

Sibling dependencies (Antibody / Alert) are documented in the repo-root `docs/DEPENDENCIES.md`.

## Notes

- Private release / prod topology lives under local `ops/` and is **not published with the repo**.
- Do not write real secrets back into `.env.example` / sample `config.yml`.
