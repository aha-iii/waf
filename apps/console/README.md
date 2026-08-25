# Nerve WAF Console

管理控制台前端（`@nerve/ui` + Vite）。

## 开发

```bash
cd Nerve-waf/Nerve-waf/apps/console
npm install
npm run dev
```

默认开发端口 `:5177`（见 `docs/PORTS.md`）；API 代理到 controller `:9000`。

## 结构

- `src/pages/*Page.tsx` — 页面
- `src/features/` — 功能模块
- `src/_legacy/` — 旧版 TS 模块（逐步淘汰）

## 构建

```bash
npm run build
```

产物输出 `dist/`，由 `nerve-waf-console` 或 Docker 镜像挂载到 `/opt/nerve-waf/console`。
