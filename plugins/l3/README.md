# L3 策略插件（网络层）

L3 插件将路由配置编译为 `SyncPlan`，经 `POST /api/sync` 下发至 Nerve Agent，写入 eBPF `nerve_policy_map`。

## 内置 Rust 插件（type=rust）

| 插件名 | 说明 |
|--------|------|
| `ip-blacklist` | IP 黑名单 → deny |
| `ip-whitelist` | IP 白名单 → allow |
| `port-block` | 按端口 deny |
| `baseline-allow` | Pod 出站基线加白 |

实现位置：`crates/engine/nerve-waf-plugins/src/`

## 在 routes.json 中使用

```json
{
  "id": "manual-blacklist",
  "name": "手动黑名单",
  "layer": "l3",
  "priority": 200,
  "enabled": true,
  "match": {},
  "plugins": [
    {
      "name": "ip-blacklist",
      "enabled": true,
      "config": { "ips": ["1.2.3.4"] }
    }
  ]
}
```

## 编译与生效

```bash
# Rust 插件需重新编译 API
cd Nerve-waf && cargo build -p nerve-waf-api --release

# 配置变更只需 Sync（无需重启 Agent）
curl -X POST http://127.0.0.1:8780/api/sync
```

## 新增 Rust 插件

1. 在 `crates/engine/nerve-waf-plugins/src/` 新建模块并实现 `WafPlugin`
2. 在 `registry.rs` 注册
3. `cargo build` 后通过 routes.json 引用插件名

外置 `plugins/l3/` 目录预留用于未来 WASM/L3 扩展；当前 L3 以 Rust 内置为准。
