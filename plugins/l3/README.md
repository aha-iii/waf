# L3 policy plugins (network)

Compile route config to `SyncPlan`, push via `POST /api/sync` to Nerve Agent eBPF (`nerve_policy_map`). Built-in Rust plugins live under `nerve-waf-plugins`.

## Built-in Rust plugins (`type=rust`)

| Plugin | Purpose |
|--------|---------|
| `ip-blacklist` | IP deny list |
| `ip-whitelist` | IP allow list |
| `port-block` | Deny by port |
| `baseline-allow` | Pod egress baseline allow |

Implementation: `crates/engine/nerve-waf-plugins/src/`

## Use in `routes.json`

```json
{
  "id": "manual-blacklist",
  "name": "manual-blacklist",
  "layer": "l3",
  "priority": 200,
  "enabled": true,
  "match": {},
  "plugins": [
    {
      "name": "ip-blacklist",
      "enabled": true,
      "config": { "ips": ["1.2.3.4"] }
    }
  ]
}
```

## Build and apply

```bash
# Rust plugins require rebuilding the API
cd Nerve-waf && cargo build -p nerve-waf-controller --release

# Config changes only need Sync (no Agent restart)
curl -X POST http://127.0.0.1:9000/api/sync
```

## Add a Rust plugin

1. Add a module under `crates/engine/nerve-waf-plugins/src/` implementing `WafPlugin`
2. Register it in `registry.rs`
3. After `cargo build`, reference the plugin name from `routes.json`

The external `plugins/l3/` tree is reserved for future WASM/L3 extensions; current L3 is built-in Rust.
