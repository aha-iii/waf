# Security model

Nerve WAF 安全边界与部署假设（架构师 / 安全评审用）。

## 1. 信任域

| 域 | 信任级别 | 暴露 |
|----|----------|------|
| **Controller API** | 高权限（策略、inspect、事件） | 仅内网 / Ingress + TLS + 鉴权 |
| **Console** | 等同 Controller（反代 API） | 内网 / VPN |
| **Agent 数据面** | 处理用户流量 | DMZ / LB 后；**不**暴露管理 API |
| **PostgreSQL** | 机密 | 仅 Controller 可达 |

## 2. 鉴权

| 机制 | 场景 |
|------|------|
| `NERVE_AUTH_ENABLED=1` + `NERVE_API_KEY` | API 自动化、inspect |
| Console 本地账号 / Keycloak（可选） | 人机管理 |

公开无需鉴权：`GET /api/health`（及 auth 登录相关路径，见 [api.md](./api.md)）。

`/api/inspect` **必须鉴权**（防规则 oracle），即使其他路径在 dev 关闭鉴权。

## 3. 网络

- 生产：`NetworkPolicy` 限制仅 Agent → Controller:9000、Controller → PG（Helm 可选模板）  
- Agent 9080 不对管理网段以外无必要开放  
- TLS：在 Agent 前终止（Ingress/LB）；Agent 9443 HTTPS 为可选能力

## 4. 密钥

| 秘密 | 存储 |
|------|------|
| `NERVE_API_KEY` | K8s Secret / Vault |
| `DATABASE_URL` | K8s Secret |
| JWT / Keycloak | Secret，勿写入 Git |

Compose 开发默认值 **禁止**用于生产（见 [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)）。

## 5. 数据

- 安全事件可能含请求片段：日志与 PG 需 retention 与访问控制  
- CRS / 规则包来自 `vendor/` 与发布流程，供应链依赖镜像签名（发布方责任）

## 6. 漏洞报告

[SECURITY.md](../SECURITY.md)

## 7. 相关

- [configuration.md](./configuration.md)  
- [concepts/boundaries.md](./concepts/boundaries.md)  
