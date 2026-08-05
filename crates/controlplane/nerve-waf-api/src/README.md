# nerve-waf-api 源码分层

```text
src/
├── controlplane/              控制面
│   ├── shared/                AppState、bootstrap、platform、中间件
│   ├── routes.rs / events.rs  Console HTTP
│   ├── ingress.rs / sites.rs  …
│   └── mod.rs                 → controlplane_router()
│
└── dataplane/                 数据面协议（控制面侧 compile + 下发）
    ├── site_inspect.rs
    ├── build.rs / generation.rs
    ├── routes.rs              /api/waf/edge/*
    └── events.rs              Edge 403 ingest
```

- **controlplane**：Console 写入、inspect、攻击日志
- **dataplane**：Edge 拉取的 snapshot（与 crate `nerve-waf-dataplane` 契约对齐）

类型契约：`crates/contract/nerve-waf-dataplane/`（独立 crate）
