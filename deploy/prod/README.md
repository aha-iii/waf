# Nerve WAF 正式部署

见设计：`docs/superpowers/specs/2026-08-03-nerve-waf-prod-deploy-design.md`

## 首次

```bash
cd Nerve-waf/Nerve-waf
cp scripts/deploy-prod.local.env.example scripts/deploy-prod.local.env
# 编辑：DEPLOY_*、POSTGRES_PASSWORD、NERVE_API_KEY、TEST_DATABASE_URL

bash scripts/deploy-prod-infra.sh
bash scripts/deploy-prod-infra.sh --sync-db   # 首次从测试库导入
bash scripts/deploy-prod.sh --up
```

## 日常发版

```bash
bash scripts/deploy-prod.sh --up
# 或跳过编译：
bash scripts/deploy-prod.sh --skip-build --up
```

## 目录

| 路径 | 用途 |
|------|------|
| `infra/` | Postgres / Redis / etcd / APISIX（一次） |
| `app/` | 业务容器（可反复） |

远端默认：`/aha/nerve/infra` + `/aha/nerve/nerve-waf`。

DNS：`waf` / `alert` / `www`.`ahatec.com.cn` → 正式机。

前置：远端已 `docker load` 镜像 `nerve/nerve-waf-api:1.0.0`（与 mars-test 相同）。
