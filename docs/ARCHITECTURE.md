# Nerve WAF — code architecture

**Runtime architecture (control/data plane, sync, HA):** [architecture-overview.md](./architecture-overview.md)  
**Product boundaries:** [concepts/boundaries.md](./concepts/boundaries.md)

Nerve WAF = **L7 detection** + **control plane API** + **L7 agent dataplane**.  
Crate prefix: `nerve-waf-*` · [NAMING.md](./NAMING.md)

## Crate layers

```text
crates/
├── contract/       Shared types (nerve-waf-sdk)
├── engine/         Detection pipeline — no DB, no HTTP
├── modules/        Pipeline plugins
├── controlplane/   Controller API, Store, CRS import
└── dataplane/      nerve-waf-agent, embed runtime, snapshots
```

Details: [crates/README.md](../crates/README.md).

## Detection pipeline

```text
HTTP request
  → normalize / canonical
  → stages: expert (optional) → core (CRS) → custom
  → verdict + optional event persistence
```

Optional `--features expert|alert`: [DEPENDENCIES.md](./DEPENDENCIES.md).

## Processes

| Binary | Crate path | Role |
|--------|------------|------|
| `nerve-waf-controller` | controlplane | API + snapshot source |
| `nerve-waf-agent` | dataplane | Sync + proxy + inspect |
| `nerve-waf-console` | controlplane bin | Static UI |

Ports: [PORTS.md](./PORTS.md).

## Configuration

`config.yml` + env — [configuration.md](./configuration.md).

Deploy: [../deploy/TOPOLOGY.md](../deploy/TOPOLOGY.md).
