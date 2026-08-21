# nerve-waf-edge source layout

The whole process **is** the dataplane; sources live under `dataplane/`:

```text
src/
├── dataplane/
│   ├── sync/       List + Watch control-plane snapshots
│   ├── runtime/    in-memory snapshot, Host routing
│   ├── inspect/    embed inspection (same path as API)
│   └── proxy/      HTTP entry, 403 report, upstream
└── main.rs
```

Control plane runs in a separate `nerve-waf-api` process under `controlplane/` + `dataplane/` (compile side).
