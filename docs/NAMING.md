# Naming (product vs crate)

| Layer | Name | Notes |
|-------|------|-------|
| **Suite** | Nerve WAF 套件 | 本仓库（Nerve WAF）+ 兄弟仓库 Nerve Antibody |
| **This repo root** | `./` | Control plane / detection / Console / edge（standalone 仓库） |
| **Product name** | Nerve WAF | Docs and external narrative |
| **Rust crates** | `nerve-waf-*` | Stable package names |
| **Binaries** | `nerve-waf-controller`, `nerve-waf-agent`, `nerve-waf-console`, … | Match crate names |

Default ports: [docs/PORTS.md](./PORTS.md) (`9000` / `9090` / `9080`).
| **Expert engine** | Nerve Antibody | 兄弟仓库 `Nerve-antibody`（与本仓库并列克隆）；wired via `nerve-antibody` |

Historically the repository/product was named `Nerve-defender`; it is now unified as **Nerve WAF**. Crate prefix is unchanged.
