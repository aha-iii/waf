# L7 detection plugins (HTTP)

Loaded by plugin-host: JSON rule packs first, then Lua `inspect(req)`. Sample packs: `owasp-sqli` / `owasp-xss` / `owasp-misc` / `custom-header-check`.

Detection chain order:

1. JSON rule packs (`nerve-waf-l7` regex engine)
2. Lua scripts (`inspect(req)`)

## Subdirectories

| Directory | type | Purpose |
|-----------|------|---------|
| `owasp-sqli/` | json | SQL injection rules |
| `owasp-xss/` | json | XSS rules |
| `owasp-misc/` | json | Path traversal / RCE / SSRF / Webshell |
| `custom-header-check/` | lua | Custom Header / URI checks |

## `plugin.json` fields

```json
{
  "name": "owasp-sqli",
  "layer": "l7",
  "type": "json",
  "version": "1.0.0",
  "category": "sqli",
  "priority": 100,
  "rules": ["rules/*.json"],
  "enabled": true
}
```

Lua plugins set `"type": "lua"` and `"script": "plugin.lua"`.

## Rule JSON format

```json
{
  "id": "sql-001",
  "name": "SQLi-UNION",
  "pattern": "(?i)union\\s+select",
  "category": "sqli",
  "severity": "high"
}
```

`category` must match a module name in `data/modules.json` (controls enablement and action).

## Hot reload

```bash
# After editing rules/*.json or plugin.lua
curl -X POST http://127.0.0.1:9000/api/plugins/reload

# Verify
curl -X POST http://127.0.0.1:9000/api/inspect \
  -H "Content-Type: application/json" \
  -d '{"uri":"/search?q=1+or+1%3D1","persist":true}'
```

## `modules.json`

Global detection mode and per-module actions live in `data/modules.json` (`mode`: `block` | `detect`).
