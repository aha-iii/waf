# nerve-waf-plugin-host

Plugin host: scans `plugins/`, loads **JSON rule packs** and **Lua scripts**, and builds the `HostedL7` detection chain.

## Responsibilities

- Parse `plugin.json` manifests
- Load `rules/*.json` and precompile regexes
- Load `plugin.lua` and call `inspect(req)`
- Merge with `nerve-waf-plugins` (Rust L3) for `GET /api/plugins`

## Dependencies

- `mlua` (Lua 5.4, vendored)
- `nerve-waf-l7` (rule engine)

## Usage

```rust
let mut host = PluginHost::new(resolve_plugins_dir(Some(data_dir)));
let report = host.reload();
let hosted = HostedL7::from_host(&host, &modules);
let hit = hosted.inspect(&req);
```

## Environment

`NERVE_WAF_PLUGINS` — plugins root directory
