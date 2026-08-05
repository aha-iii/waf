# owasp-sqli

SQL 注入 JSON 规则包，**无需编译**，改 `rules/*.json` 后 reload 生效。

```bash
curl -X POST http://127.0.0.1:8780/api/plugins/reload
```

模块开关：`data/modules.json` → `modules.sqli`
