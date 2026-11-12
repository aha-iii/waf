#Requires -RunAsAdministrator
#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$Clear,
    [Alias("Remove")]
    [switch]$ClearProxy,
    [string]$WslIp = ""
)

$ErrorActionPreference = "Stop"

# WAF 三进程端口：Windows 主机 → WSL2（netsh portproxy + 防火墙）
#
#   管理员 PowerShell：
#   powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1
#   powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1 -Clear
#   powershell -ExecutionPolicy Bypass -File .\scripts\wsl-port-forward.ps1 -WslIp 172.28.x.x
#
# 默认映射：9000 controller · 9080 agent · 9090 console
#
# 说明：参数不用 -Remove，避免与会话变量 $Remove（常为字符串）冲突。

$doClear = $Clear -or $ClearProxy

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../../..")).Path
$Shared = Join-Path $RepoRoot "scripts/wsl-port-forward.ps1"
if (-not (Test-Path -LiteralPath $Shared)) {
    throw "missing shared script: $Shared"
}

$ports = @(9000, 9080, 9090)

if ($doClear) {
    if ($WslIp) {
        & $Shared -Remove -Ports $ports -WslIp $WslIp
    } else {
        & $Shared -Remove -Ports $ports
    }
} elseif ($WslIp) {
    & $Shared -Ports $ports -WslIp $WslIp
} else {
    & $Shared -Ports $ports
}
