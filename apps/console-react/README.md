# Nerve WAF Console（React）

基于 `@nerve/ui` 重写的 Console。**原 `apps/console` 保留不动。**

| | 原 Console | 本项目 |
|--|--|--|
| 路径 | `apps/console` | `apps/console-react` |
| 技术 | 原生 TS 拼 HTML | React + `@nerve/ui` |
| 端口 | 5174 | **5177** |

```bash
cd Nerve-waf/apps/console-react
npm install
npm run dev   # 监听 0.0.0.0:5177
```

宿主机浏览器打开：`http://127.0.0.1:5177`（WSL2 一般会自动转发 localhost）。

要用 **Windows 局域网 IP**（如 `http://192.168.74.15:5177`）访问时，须把 WSL 端口映射到主机。在 **管理员 PowerShell** 执行：

```powershell
cd d:\workspace\waf
powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1
```

WSL 重启后 IP 可能变化，需再跑一次。
需要 **Node ≥ 18**（推荐 20）。当前环境若是 Node 16，`vite build` 会失败，但 `npx tsc --noEmit` 已通过。

## 结构

- `src/pages/*Page.tsx` — 全部导航页（React + Nerve* 组件）
- `src/layout` / `src/hooks` — 壳与 hash 路由
- `src/lib` — 类型与共享逻辑
- `src/_legacy` — 原 Console 拷贝的旧视图（**不参与编译**，仅对照）

## 说明

- API 代理与原 Console 相同（可与 `:5174` 并行）
- 「应用配置」抽屉等超大交互仍有占位提示；列表/策略/规则/运营页已用 `@nerve/ui`
- 包依赖：`"@nerve/ui": "file:../../../Nerve-ui"`
