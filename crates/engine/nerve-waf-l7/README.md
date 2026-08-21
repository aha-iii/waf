# nerve-waf-l7

L7 HTTP detection engine core. Rule sources:

1. **External**: `plugins/l7/*/rules/*.json` (loaded by plugin-host; hot reload)
2. **Fallback**: embedded `rules/*.json` when the plugins directory is empty

## API

```rust
let engine = L7Engine::from_rules(modules, compiled_rules);
engine.inspect(&InspectRequest { uri: "/?id=1 or 1=1".into(), ..Default::default() });
```

## Rule format

See `plugins/l7/README.md`.

## Build

```bash
cargo test -p nerve-waf-l7
cargo build -p nerve-waf-l7
```

At runtime the Control API assembles this via plugin-host; you usually do not run it alone.
