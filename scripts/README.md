# Scripts（不上 GitLab）

本目录下的**测试/正式打包与上传脚本仅保留在本地**，已由仓库根 `.gitignore` 忽略（`/scripts/*`），**不会**随 `git add` / 推送到 GitLab。

本地仍可使用（需本机有这些文件）：

```bash
bash scripts/deploy-test.sh
bash scripts/deploy-prod-infra.sh
bash scripts/deploy-prod.sh --up
```

若同事需要发版脚本，请走内部私有渠道同步，不要提交进公开/GitLab 产品仓。
