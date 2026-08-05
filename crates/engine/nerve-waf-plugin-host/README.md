# nerve-waf-plugin-host

插件宿主：扫描 `plugins/`，加载 **JSON 规则包** 与 **Lua 脚本**，组装 `HostedL7` 检测链。

## 职责

- 解析 `plugin.json` 清单
- 加载 `rules/*.json` 并预编译正则
- 加载 `plugin.lua` 并调用 `inspect(req)`
- 与 `nerve-waf-plugins`（Rust L3）合并列表 `GET /api/plugins`

## 依赖

- `mlua`（Lua 5.4，vendored）
- `nerve-waf-l7`（规则引擎）

## 使用

```rust
let mut host = PluginHost::new(resolve_plugins_dir(Some(data_dir)));
let report = host.reload();
let hosted = HostedL7::from_host(&host, &modules);
let hit = hosted.inspect(&req);
```

## 环境变量

`NERVE_WAF_PLUGINS` — 插件根目录
