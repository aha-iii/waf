# nerve-waf-api source layout

```text
src/
├── controlplane/              control plane
│   ├── shared/                AppState, bootstrap, platform, middleware
│   ├── routes.rs / events.rs  Console HTTP
│   ├── ingress.rs / sites.rs  …
│   └── mod.rs                 → controlplane_router()
│
└── dataplane/                 dataplane protocol (control-side compile + push)
    ├── site_inspect.rs
    ├── build.rs / generation.rs
    ├── routes.rs              /api/waf/edge/*
    └── events.rs              Edge 403 ingest
```

- **controlplane**: Console writes, inspect, attack logs
- **dataplane**: snapshots Edge pulls (aligned with crate `nerve-waf-dataplane`)

Type contract: `crates/contract/nerve-waf-dataplane/` (separate crate)
