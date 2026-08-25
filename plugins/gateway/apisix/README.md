# APISIX + Nerve WAF integration

Copy `nerve-waf.lua` into the APISIX plugins directory. After the route mounts the plugin, each request `POST`s to WAF `:9000/api/inspect` (HTTP bypass path; coexists with the Agent Unix socket path). Change admin keys and WAF URLs before use.

## 1. Install the plugin

```bash
# Assuming APISIX at /usr/local/apisix
cp nerve-waf.lua /usr/local/apisix/apisix/plugins/nerve-waf.lua
```

Edit `conf/config.yaml` and add to the `plugins:` list:

```yaml
plugins:
  - nerve-waf
  # ... other plugins
```

Reload:

```bash
apisix reload
# or docker: docker exec apisix apisix reload
```

## 2. Create a test route

```bash
ADMIN=http://127.0.0.1:9180
KEY=change-me-apisix-admin-key   # must match your APISIX admin key

curl -s "$ADMIN/apisix/admin/routes/nerve-waf-demo" \
  -H "X-API-KEY: $KEY" -X PUT -d '{
  "uri": "/*",
  "upstream": {
    "type": "roundrobin",
    "nodes": { "127.0.0.1:9080": 1 }
  },
  "plugins": {
    "nerve-waf": {
      "waf_api": "http://127.0.0.1:9000",
      "api_key": "",
      "app_id": "default",
      "block_on_hit": true,
      "fail_open": false,
      "persist_on_hit": true,
      "timeout": 2000
    }
  }
}'
```

**Docker note:** From an APISIX container to host WAF, use `http://host.docker.internal:9000` or `--network host`.

## 3. Attack tests

APISIX data plane defaults to `:9080` (depends on install):

```bash
GW=http://127.0.0.1:9080

# SQL injection — expect 403
curl -i "$GW/search?q=1+or+1%3D1"

# XSS
curl -i "$GW/x?q=%3Cscript%3Ealert(1)%3C/script%3E"

# Path traversal
curl -i "$GW/file?p=../../../etc/passwd"

# Benign request — should not be 403
curl -i "$GW/api/health"
```

## 4. Verify persistence (Console)

```bash
curl -s 'http://127.0.0.1:9000/api/events?limit=5&category=sqli' | jq '.events[0]'
```

In Console: Security ops → Attack logs → refresh, filter SQL injection.

## 5. Plugin options

| Field | Default | Meaning |
|-------|---------|---------|
| `waf_api` | `http://127.0.0.1:9000` | WAF Controller API |
| `api_key` | empty | Required if WAF API Key auth is enabled |
| `app_id` | `default` | Protected app id (Console app id; default uses system rules) |
| `block_on_hit` | `true` | Block on hit |
| `fail_open` | `false` | Allow traffic when WAF is unavailable |
| `persist_on_hit` | `true` | Persist hits to the event store (visible in Console) |
| `timeout` | `2000` | inspect timeout (ms) |
| `block_status` | `403` | Block HTTP status |
| `block_body` | `Blocked by Nerve WAF` | Response body prefix |

## 6. Prerequisites

- Nerve WAF Controller on `:9000` is running
- `plugins/l7/` rules are loaded (`POST /api/plugins/reload`)
- APISIX can reach the WAF address
