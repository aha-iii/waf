# Naming (product vs crate)

| Layer | Name | Notes |
|-------|------|-------|
| **Suite root** | `Nerve-waf/` | This product + Antibody |
| **This product dir** | `Nerve-waf/Nerve-waf/` | Control plane / detection / Console / edge |
| **Product name** | Nerve WAF | Docs and external narrative |
| **Rust crates** | `nerve-waf-*` | Stable package names |
| **Binaries** | `nerve-waf-api`, `nerve-waf-edge`, … | Match crate names |
| **Expert engine** | Nerve Antibody | `Nerve-waf/Nerve-antibody/`; wired via `nerve-antibody` |

Historically the directory was named `Nerve-defender`; it is now unified as **`Nerve-waf/Nerve-waf`**. Crate prefix is unchanged.
