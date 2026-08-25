# Nerve WAF 默认端口

**控制面 900x · 数据面 908x · Console 9090**。部署架构见 [deploy/TOPOLOGY.md](../deploy/TOPOLOGY.md)；生产排障见 [runbooks/PRODUCTION.md](./runbooks/PRODUCTION.md)。

| 进程 | 默认端口 | 主要 env | 用途 |
|------|----------|----------|------|
| **nerve-waf-agent** | **9080**（HTTP）· **9443**（HTTPS，可选） | `WAF_AGENT_PORT` · `WAF_AGENT_TLS_PORT` | L7 数据面：检测 + 反代 |
| **nerve-waf-controller** | **9000** | `PORT` / `WAF_CONTROLLER_PORT` | 控制面 API、快照下发 |
| **nerve-waf-console** | **9090** | `WAF_CONSOLE_PORT` / `CONSOLE_PORT` | Console 静态 UI |

生产环境公网入口通常为 **80/443**（前置 TLS）；Agent 9080/9443 多在内网或 NodePort。

## 健康检查

```text
Controller  http://127.0.0.1:9000/api/health
Console       http://127.0.0.1:9090/
Agent         http://127.0.0.1:9080/health
```

## 同机部署注意

| 端口 | 说明 |
|------|------|
| **9000** | 若被本机其它服务占用（如 ClickHouse），dev 设 `PORT=9002` 或使用 `scripts/start-waf.sh` 自动避让 |
| **9080** | 若同机其它进程已监听 9080，设 `WAF_AGENT_PORT` |
| **9090** | 若与 Prometheus 等冲突，设 `CONSOLE_PORT` |

Agent 拉取配置：`WAF_AGENT_CONTROLLER=http://<controller-host>:9000`

## WSL → Windows 端口映射

WSL2 内服务监听 `0.0.0.0` 时，从 Windows 局域网 IP 访问需 `netsh portproxy`（WSL 重启后 IP 可能变化）。

产品根 `Nerve-waf/Nerve-waf`，**管理员 PowerShell**：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/wsl-port-forward.ps1
powershell -ExecutionPolicy Bypass -File scripts/wsl-port-forward.ps1 -Clear
```

一键启动并映射：

```powershell
powershell -File scripts/start-waf.ps1 -PortForward -Build
```
