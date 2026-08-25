# Publishable ops scripts for Nerve WAF (open-source tree).

| Script | Purpose |
|--------|---------|
| [`apply-migrations.sh`](./apply-migrations.sh) | Apply PostgreSQL DDL from `crates/controlplane/nerve-waf-store/migrations/` |
| [`smoke-test.sh`](./smoke-test.sh) | Post-install health check (Controller + Agent) |
| [`start-waf.sh`](./start-waf.sh) | WSL/Linux 本地启动 controller + agent + console（见 `--help`） |
| [`wsl-port-forward.ps1`](./wsl-port-forward.ps1) | Windows 管理员：9000/9080/9090 映射到 WSL |

Maintainer-only release/deploy scripts live under local `ops/` (not in Git).

```bash
export DATABASE_URL=postgres://postgres:CHANGE_ME@127.0.0.1:5432/nerve
bash scripts/apply-migrations.sh

# 三进程联调（WSL）
bash scripts/start-waf.sh --build
bash scripts/start-waf.sh --stop
```

Windows（**管理员** PowerShell / CMD；路径含盘符时请用 `cd /d` 或下面 .cmd）：

```powershell
cd /d d:\workspace\waf\Nerve-waf\Nerve-waf
powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1
```

或任意目录双击/运行（推荐）：

```cmd
d:\workspace\waf\Nerve-waf\Nerve-waf\scripts\wsl-port-forward.cmd
```

```powershell
# 启动 + 映射
d:\workspace\waf\Nerve-waf\Nerve-waf\scripts\start-waf.cmd -PortForward -Build
```
