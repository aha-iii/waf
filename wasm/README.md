# Coraza Proxy-Wasm assets

Edge single-process CRS uses the [coraza-proxy-wasm](https://github.com/corazawaf/coraza-proxy-wasm) build artifact.

| File | Description |
|------|-------------|
| `coraza-proxy-wasm.wasm` | Official release (OWASP CRS v4 embedded) |

Download / update:

```bash
bash Nerve-waf/scripts/download-coraza-wasm.sh
```

Environment variables:

- `NERVE_WAF_WASM_CRS_PATH` — override wasm path
- `NERVE_WAF_WASM_CRS_CONFIG` — Coraza plugin JSON (default: full CRS)
- `NERVE_WAF_CRS_ENGINE=wasm` — enable explicitly (default on when wasm exists and unset)
