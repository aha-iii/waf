# Nerve WAF architecture

Product name **Nerve WAF** (directory `Nerve-waf/`). This product = **L7 detection** + **control plane** + **WAF-L3 config (pushed to Agent eBPF)** + **edge dataplane**.  
Crate prefix remains `nerve-waf-*`; see [NAMING.md](./NAMING.md). Sibling deps: [DEPENDENCIES.md](./DEPENDENCIES.md).

Platform topology / cluster NetworkPolicy belong to **Nerve-control** (`:8780`), not a long-term responsibility of this repo.

## Relation to ecosystem components

| Product | Location | Role |
|---------|----------|------|
| **Nerve WAF** | `Nerve-waf/Nerve-waf/` | L7 inspect, events, WAF-L3, Console, edge |
| **Nerve Antibody** | `Nerve-waf/Nerve-antibody/` | Expert engine + rule packs (inspect `expert`) |
| **Nerve Gateway** | `Nerve-gateway/` | L7 routing / reverse proxy / call `/api/inspect` |
| **Nerve Agent** | `Nerve/` | eBPF enforcement of WAF-L3 |
| **Nerve-alert** | `Nerve-alert/` | Alert center (API reports via client) |

## Workspace layers

See `crates/README.md` for the contract / engine / modules / controlplane / dataplane split.

## Detection pipeline

```text
HTTP request
  → normalize / canonical
  → pipeline stages:
      expert     : Antibody (needs sibling repo)
      core       : CRS / signatures
      custom     : custom / api-sec
  → optional event persistence + Alert dispatch
```

## Default ports

| Service | Port |
|---------|------|
| Control API | `8781` (examples may also use `8780`) |
| Console (Vite) | `5177` |
| Edge | `8080` |

Config: `config.yml` + `.env` (see `.env.example`).
