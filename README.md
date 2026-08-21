# Nerve WAF

L7 inspection pipeline, control-plane API, WAF Console, and edge dataplane.  
Path: `Nerve-waf/Nerve-waf` · crate prefix: `nerve-waf-*` (see [docs/NAMING.md](./docs/NAMING.md)).

| Component | Default port | Notes |
|-----------|--------------|-------|
| Control API | `:8781` | `nerve-waf-api` |
| Console | Vite dev / or hosted by API | `apps/console-react` |
| Edge | `:8080` | `nerve-waf-edge` (Docker example) |

Docs: [DEPENDENCIES](./docs/DEPENDENCIES.md) · [ARCHITECTURE](./docs/ARCHITECTURE.md) · [CONTRIBUTING](./CONTRIBUTING.md) · [SECURITY](./SECURITY.md) · [deploy/START.md](./deploy/START.md)

## Quick start

### A. Docker (recommended first)

```bash
cd deploy
cp .env.example .env
# edit DATABASE_URL=postgres://postgres:CHANGE_ME@127.0.0.1:5432/nerve
docker compose up -d
```

Open `http://127.0.0.1:8781/api/health`. Details: [deploy/START.md](./deploy/START.md).

### B. From source

```bash
cp .env.example .env
# set DATABASE_URL / NERVE_API_KEY as needed
cargo run -p nerve-waf-api --release
cd apps/console-react && npm install && npm run dev
```

Sibling dependency layout: [docs/DEPENDENCIES.md](./docs/DEPENDENCIES.md).

## Layout

```text
crates/…          # engine + control plane
apps/console-react/
deploy/           # shared Docker / Helm / sample config (publishable)
ops/              # maintainer-local release materials (not in Git)
```

## License

[MIT](./LICENSE)
