# Coraza Proxy-Wasm 资产

Edge 单进程 CRS 使用 [coraza-proxy-wasm](https://github.com/corazawaf/coraza-proxy-wasm) 编译产物。

| 文件 | 说明 |
|------|------|
| `coraza-proxy-wasm.wasm` | 官方 release（含 OWASP CRS v4 内嵌规则） |

下载/更新：

```bash
bash Nerve-waf/scripts/download-coraza-wasm.sh
```

环境变量：

- `NERVE_WAF_WASM_CRS_PATH` — 覆盖 wasm 路径
- `NERVE_WAF_WASM_CRS_CONFIG` — Coraza 插件 JSON（默认加载全集 CRS）
- `NERVE_WAF_CRS_ENGINE=wasm` — 显式启用（未设置且 wasm 存在时默认启用）
