# Scripts 约定

## 会入库（开源 / CI / 本地开发）

| 类型 | 示例 |
|------|------|
| 启动 | `start-*.sh`、`start-api.sh` |
| 依赖与规则 | `download-*.sh`、`import-crs.sh`、`npm-deps.sh`、`fetch-sibling-deps.sh`、`check-sibling-deps.sh` |
| 冒烟 / 回归 | `defense-smoke-*`、`e2e-*`、`gotestwaf-*`、`run-waf-bench-*`、`run-gotestwaf-*` |
| 构建辅助 | `docker-build-console.sh`、`build-with-hyperscan.sh`、`pack-dist-runtime*`、`init-db.sql` |
| 说明 | `README.md` |

## 不入库（运维私有，已在根 `.gitignore`）

本地可继续放在 `scripts/`，但 **不要 `git add`**：

- `diag-*`、`deploy-prod*`、`deploy-runtime*`、`deploy-test*`、`deploy-mars*`
- `emergency-*`、`ensure-prod*`、`migrate-prod*`、`finish-tls*`、`reload-prod*`
- `attack-*`、`ssh-smoke*`、`tune-gitlab*`、`_*` 临时脚本
- 各类 `fix-*-corruption*` / 环境特供 fix、以及 `scripts/lib/deploy-*.sh`

这类脚本请放到私有 ops 仓，或仅留在本机。
