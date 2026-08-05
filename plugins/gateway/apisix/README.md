# APISIX + Nerve WAF 联调

将 `nerve-waf.lua` 复制到 APISIX 插件目录，路由挂载后，每个请求会 `POST` 到 WAF `:8780/api/inspect`（**HTTP 旁路入口，与 Agent 本地 Unix Socket 路径并存**）。

## 1. 安装插件

```bash
# 假设 APISIX 在 /usr/local/apisix
cp nerve-waf.lua /usr/local/apisix/apisix/plugins/nerve-waf.lua
```

编辑 `conf/config.yaml`，在 `plugins:` 列表增加：

```yaml
plugins:
  - nerve-waf
  # ... 其他插件
```

重载：

```bash
apisix reload
# 或 docker: docker exec apisix apisix reload
```

## 2. 创建测试路由

```bash
ADMIN=http://127.0.0.1:9180
KEY=edd1c9f034335f136f87ad84b625c8f1   # 改成你的 admin key

curl -s "$ADMIN/apisix/admin/routes/nerve-waf-demo" \
  -H "X-API-KEY: $KEY" -X PUT -d '{
  "uri": "/*",
  "upstream": {
    "type": "roundrobin",
    "nodes": { "127.0.0.1:8080": 1 }
  },
  "plugins": {
    "nerve-waf": {
      "waf_api": "http://127.0.0.1:8780",
      "api_key": "",
      "app_id": "default",
      "block_on_hit": true,
      "fail_open": false,
      "persist_on_hit": true,
      "timeout": 2000
    }
  }
}'
```

**Docker 注意：** APISIX 容器内访问宿主机 WAF 时用 `http://host.docker.internal:8780` 或 `--network host`。

## 3. 攻击测试

APISIX 数据面默认 `:9080`（视安装而定）：

```bash
GW=http://127.0.0.1:9080

# SQL 注入 — 期望 403
curl -i "$GW/search?q=1+or+1%3D1"

# XSS
curl -i "$GW/x?q=%3Cscript%3Ealert(1)%3C/script%3E"

# 路径穿越
curl -i "$GW/file?p=../../../etc/passwd"

# 正常请求 — 不应 403
curl -i "$GW/api/health"
```

## 4. 验证入库（Console 大盘）

```bash
curl -s 'http://127.0.0.1:8780/api/events?limit=5&category=sqli' | jq '.events[0]'
```

Console → 安全运营 → 攻击日志 → 刷新，筛选「SQL 注入」。

## 5. 插件配置项

| 字段 | 默认 | 说明 |
|------|------|------|
| `waf_api` | `http://127.0.0.1:8780` | WAF Control API |
| `api_key` | 空 | 若 WAF 开启 API Key 鉴权 |
| `app_id` | `default` | 防护应用 ID（对应 Console 应用 ID；缺省走系统默认规则） |
| `block_on_hit` | `true` | 命中则拦截 |
| `fail_open` | `false` | WAF 不可用时是否放行 |
| `persist_on_hit` | `true` | 命中写入事件库（大盘可见） |
| `timeout` | `2000` | inspect 超时 ms |
| `block_status` | `403` | 拦截 HTTP 状态码 |
| `block_body` | `Blocked by Nerve WAF` | 响应正文前缀 |

## 6. 前置条件

- Nerve WAF Control API `:8780` 已启动
- `plugins/l7/` 规则已加载（`POST /api/plugins/reload`）
- APISIX 能访问 WAF 地址（网络互通）
