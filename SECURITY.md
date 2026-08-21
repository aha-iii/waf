# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| `1.x` (workspace `Cargo.toml`) | ✅ Active development |
| Older untagged trees | ❌ No security backports promised |

## Reporting a vulnerability

**Do not** open a public GitHub/GitLab issue for security bugs (RCE, auth bypass, rule-bypass that enables exploitation of protected apps, secret leakage, etc.).

Please report privately to the maintainers via your organization’s security contact
(replace this sentence with a real email or private tracker before making the repo public).

Include:

1. Affected component (`nerve-waf-api`, edge, inspect pipeline, Console, …)
2. Version / commit hash
3. Reproduction steps (PoC against a **local** Nerve WAF instance only)
4. Impact assessment

You should receive an acknowledgement within **72 hours**. We aim to publish a fix or mitigation advisory after coordinated disclosure.

## Scope notes

- False positives / false negatives in detection rules are usually **not** security vulnerabilities of Nerve WAF itself; prefer rule/pack issues under Antibody packs.
- Production deploy scripts under `scripts/` may assume private infra; do not treat them as a hardened public surface.

## Preferred hardening baselines

- Keep `NERVE_AUTH_ENABLED=1` and rotate `NERVE_API_KEY` / JWT secrets
- Prefer `mode=edge` with fail-closed inspect when protecting production origins
- Do not commit `.env`, TLS private keys, or `*.local.env`
