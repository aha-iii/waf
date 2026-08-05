# L7 检测插件（HTTP 层）

L7 插件由 **plugin-host** 加载，检测链顺序：

1. JSON 规则包（`nerve-waf-l7` 正则引擎）
2. Lua 脚本（`inspect(req)` 函数）

## 子目录

| 目录 | type | 说明 |
|------|------|------|
| `owasp-sqli/` | json | SQL 注入规则 |
| `owasp-xss/` | json | XSS 规则 |
| `owasp-misc/` | json | 路径穿越/RCE/SSRF/Webshell |
| `custom-header-check/` | lua | 自定义 Header/URI 检查 |

## plugin.json 字段

```json
{
  "name": "owasp-sqli",
  "layer": "l7",
  "type": "json",
  "version": "1.0.0",
  "category": "sqli",
  "priority": 100,
  "rules": ["rules/*.json"],
  "enabled": true
}
```

Lua 插件将 `"type": "lua"` 并设置 `"script": "plugin.lua"`。

## 规则 JSON 格式

```json
{
  "id": "sql-001",
  "name": "SQL注入-UNION",
  "pattern": "(?i)union\\s+select",
  "category": "sqli",
  "severity": "high"
}
```

`category` 须与 `data/modules.json` 中模块名一致（控制开关与 action）。

## 热更新

```bash
# 改 rules/*.json 或 plugin.lua 后
curl -X POST http://127.0.0.1:8780/api/plugins/reload

# 验证
curl -X POST http://127.0.0.1:8780/api/inspect \
  -H "Content-Type: application/json" \
  -d '{"uri":"/search?q=1+or+1%3D1","persist":true}'
```

## modules.json

全局检测模式与各模块 action 见 `data/modules.json`（`mode`: `block` | `detect`）。
