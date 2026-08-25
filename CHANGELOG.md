# Changelog

All notable changes to **Nerve WAF** (`Nerve-waf/Nerve-waf`) are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-23

### Added

- **architecture-overview.md** — single source for runtime architecture, sync, failure domains.
- **concepts/boundaries.md**, **ha-deployment.md**, **security-model.md**, **openapi.yaml**.
- Helm: `agent.mode` deployment/daemonset, optional NetworkPolicy & ServiceMonitor.
- Open-source deployment stack: Docker Compose, Helm, CI/Release workflows.
- `Makefile`, `scripts/smoke-test.sh`, community templates.

### Changed

- **ARCHITECTURE.md** refocused on code layers; ecosystem deps moved to boundaries doc.
- Helm values restructured (`controller.*`, `agent.*`) — see [UPGRADING.md](./UPGRADING.md).

[1.0.0]: https://github.com/YOUR-ORG/YOUR-REPO/releases/tag/nerve-waf-v1.0.0
