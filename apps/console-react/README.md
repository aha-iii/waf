# Nerve WAF Console (React)

Console rewrite on `@nerve/ui`. The legacy `apps/console` tree is left unchanged.

| | Legacy Console | This app |
|--|--|--|
| Path | `apps/console` | `apps/console-react` |
| Stack | Vanilla TS + HTML | React + `@nerve/ui` |
| Port | 5174 | **5177** |

```bash
cd Nerve-waf/apps/console-react
npm install
npm run dev   # listens on 0.0.0.0:5177
```

Open on the host: `http://127.0.0.1:5177` (WSL2 usually forwards localhost automatically).

To reach a **Windows LAN IP** (e.g. `http://192.168.74.15:5177`), map the WSL port on the host. In an **Administrator PowerShell**:

```powershell
cd d:\workspace\waf
powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1
```

WSL IP may change after restart; re-run the script then. Requires **Node ≥ 18** (20 recommended). On Node 16, `vite build` fails; `npx tsc --noEmit` still passes in current setups.

## Layout

- `src/pages/*Page.tsx` — navigable pages (React + Nerve* components)
- `src/layout` / `src/hooks` — shell and hash routing
- `src/lib` — types and shared logic
- `src/_legacy` — copy of legacy Console views (**not compiled**; reference only)

## Notes

- API proxy matches the legacy Console (can run alongside `:5174`)
- Large interactions such as the app-config drawer may still show placeholders; list / strategy / rules / ops pages use `@nerve/ui`
- Package dependency: `"@nerve/ui": "file:../../../Nerve-ui"`
