# nerve-waf-edge 源码分层

整个进程即 **数据面**；源码统一在 `dataplane/` 下：

```text
src/
├── dataplane/
│   ├── sync/       List + Watch 拉控制面 snapshot
│   ├── runtime/    内存快照、Host 路由
│   ├── inspect/    embed 检测（与 API 同路径）
│   └── proxy/      HTTP 入口、403 上报、回源
└── main.rs
```

控制面在独立进程 `nerve-waf-api` 的 `controlplane/` + `dataplane/`（compile 侧）。
