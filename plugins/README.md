# Nerve WAF plugins directory

Hot-reloadable external plugins under `l3/` and `l7/`. Scanned when the control plane starts, or on `POST /api/plugins/reload` / `POST /api/sync`. Env: `NERVE_WAF_PLUGINS`.

## Layout

```
plugins/
├── README.md          ← this file
├── l3/                ← L3 network policy (built-in Rust + route config)
└── l7/                ← L7 HTTP detection (JSON rule packs + Lua scripts)
```

## Plugin types

| type | Language | Hot reload | Purpose |
|------|----------|------------|---------|
| `rust` | Rust | Rebuild `nerve-waf-plugins` | L3 deny/allow/baseline (core) |
| `json` | JSON rules | Edit files + reload | OWASP rule packs |
| `lua` | Lua 5.4 | Edit script + reload | Custom detection logic |

## Environment

- `NERVE_WAF_PLUGINS` — plugins root (default `Nerve-waf/plugins`)

## Apply flow

```
1. Edit plugins/l7/... or data/routes.json / data/modules.json
2. POST http://localhost:8780/api/plugins/reload   # reload L7 rules/Lua only
   or POST http://localhost:8780/api/sync          # L3 push to Agent + reload L7
3. Next POST /api/inspect or Gateway request uses the new rules
```

## API

- `GET /api/plugins` — list Rust + JSON + Lua plugins
- `POST /api/plugins/reload` — hot-reload the plugins/ tree

## Add a JSON rule pack

```bash
mkdir -p plugins/l7/my-pack/rules
# Add plugin.json + rules/*.json
curl -X POST http://127.0.0.1:8780/api/plugins/reload
```

## Add a Lua plugin

See the sample under `plugins/l7/custom-header-check/`.

## Add a Rust L3 plugin

See `crates/engine/nerve-waf-plugins/README.md`. After build it appears in `GET /api/plugins` with `type=rust`.
