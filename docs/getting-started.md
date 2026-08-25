# Getting started

安装完成后按本节验证 Nerve WAF 是否正常工作。安装步骤见 [deploy/START.md](../deploy/START.md)。

## 1. 健康检查

默认端口（可通过环境变量修改，见 [PORTS.md](./PORTS.md)）：

```bash
export CONTROLLER_URL="${CONTROLLER_URL:-http://127.0.0.1:9000}"
export AGENT_URL="${AGENT_URL:-http://127.0.0.1:9080}"

curl -fsS "$CONTROLLER_URL/api/health"
curl -fsS "$AGENT_URL/health"
```

或使用仓库脚本：

```bash
bash scripts/smoke-test.sh
```

## 2. 登录 Console（若已部署）

Console 默认 **9090**（Docker Compose 全栈或 `start-waf.sh`）。

- 开发环境 Compose 常设 `NERVE_AUTH_ENABLED=0`，可直接打开 UI。
- 生产环境 `NERVE_AUTH_ENABLED=1` 时，使用 Console 登录页或 API Key（见 [configuration.md](./configuration.md)）。

## 3. 检测验证（需 API 鉴权时）

当 `NERVE_AUTH_ENABLED=1`，inspect 接口需要 `X-API-Key` 或登录态：

```bash
curl -fsS -X POST "$CONTROLLER_URL/api/inspect" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: ${NERVE_API_KEY:-dev-nerve-key}" \
  -d '{
    "method": "GET",
    "uri": "/search?q=1+or+1%3D1",
    "headers": {}
  }'
```

预期：命中规则时响应含 `action` / `blocked` 等字段（视策略而定）。  
开发 Compose（`NERVE_AUTH_ENABLED=0`）可省略 `X-API-Key`。

## 4. 下一步

- 配置持久化与生产清单：[runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)
- 配置项详解：[configuration.md](./configuration.md)
- Kubernetes：[deploy/helm/nerve-waf/README.md](../deploy/helm/nerve-waf/README.md)
