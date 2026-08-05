# Nerve WAF 测试部署

## 直接启动（两步）

```powershell
cd D:\workspace\waf\Nerve-waf\deploy

# 1. 加载镜像（只需一次）
docker load -i ..\dist\nerve-waf-api-1.0.0.tar

# 2. 启动
docker compose up -d
```

或一键脚本：

```powershell
powershell -File scripts\start.ps1
```

验证：

- 管理面：`http://127.0.0.1:8781/`（Console）· `http://127.0.0.1:8781/api/health`
- 数据面：`http://127.0.0.1:8080/health` · 业务 Host 回源测 `8080`

## 文件说明

| 文件 | 作用 |
|------|------|
| `docker-compose.yml` / `docker-compose.yaml` | `nerve-waf-api`（8781）+ `nerve-waf-edge`（8080） |
| `.env` | 环境变量（鉴权关、端口 8781） |
| `config/config.yml` | WAF 配置（挂载进容器） |

## 可选

- 改端口：编辑 `.env` 中 `PORT`
- 关 native 引擎：`.env` 设 `NERVE_EXPERT_ENGINE=off`
- 构建 tar：`powershell -File scripts\build-image.ps1`

Helm / 生产部署见下文历史章节。
