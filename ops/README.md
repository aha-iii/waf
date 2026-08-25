# ops/（维护者本机，不上 Git）

私有发版、正式/测试机编排与证书。**已被 `.gitignore` 的 `/ops/*` 忽略**（仅本 README 可提交）。

外人克隆仓库后**不需要**本目录；请按根 [README.md](../README.md) 与 [deploy/START.md](../deploy/START.md) 用通用示例部署。

## 布局（本机）

| 路径 | 说明 |
|------|------|
| `scripts/` | 打包与上传 |
| `scripts/start-waf.sh` | WSL 本地联调：controller + agent + console |
| `scripts/wsl-port-forward.ps1` | Windows 管理员：9000/9080/9090 → WSL |
| `deploy-scripts/` | 镜像构建 / 本地 compose 助手 |
| `infra-scripts/` | 网关 / TLS 助手 |
| `prod/` | 正式环境编排 |
| `private-test/` | 私有测试环境编排（本机目录名以实际为准） |

```bash
bash ops/scripts/start-waf.sh --build    # 或 bash scripts/start-waf.sh
bash ops/scripts/deploy-test.sh
bash ops/scripts/deploy-prod.sh --up
```
