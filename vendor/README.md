# vendor/

| 路径 | 说明 |
|------|------|
| `coreruleset/` | [OWASP CRS](https://github.com/coreruleset/coreruleset) 快照，供导入 / 对照；许可证见其目录内 `LICENSE` |
| `proxy-wasm-test-framework/` | Proxy-WASM 测试辅助 |
| `blazehttp` | 本地数据集软链（**不入库**，见根 `.gitignore`） |

## 卫生约定

- 不要把本仓的 CI/Issue 模板改动混进 `vendor/coreruleset/.github/`（该目录已 gitignore）
- 升级 CRS：替换快照并记录版本；业务规则优先走 Antibody packs / `nerve-waf-crs-import`
- `dist/` 为打包产物，不入库；发行请用 `scripts/pack-dist-runtime*` 生成后单独发布
