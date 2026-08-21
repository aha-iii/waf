# owasp-sqli

SQL injection JSON rule pack (no compile step). Edit `rules/*.json`, then `POST /api/plugins/reload`.

```bash
curl -X POST http://127.0.0.1:8780/api/plugins/reload
```

Module switch: `data/modules.json` → `modules.sqli`
