# Coraza Proxy-Wasm assets

Edge single-process CRS uses the [coraza-proxy-wasm](https://github.com/corazawaf/coraza-proxy-wasm) build artifact.

| File | Description |
|------|-------------|
| `coraza-proxy-wasm.wasm` | Official release (OWASP CRS v4 embedded) |

`coraza-proxy-wasm.wasm`（约 18 MB）已随仓库提供，无需额外下载。
需要换版本时，用上游 release 的产物覆盖本目录同名文件即可。

> 构建期的下载脚本属内部工具，未随仓库发布。

Environment variables:

- `NERVE_WAF_WASM_CRS_PATH` — override wasm path
- `NERVE_WAF_WASM_CRS_CONFIG` — Coraza plugin JSON (default: full CRS)
- `NERVE_WAF_CRS_ENGINE=wasm` — enable explicitly (default on when wasm exists and unset)
