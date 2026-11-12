#Requires -Version 5.1
$ErrorActionPreference = "Stop"

# Windows 入口：在 WSL 内启动 controller + agent + console；可选先做端口映射
#
#   powershell -File Nerve-waf/Nerve-waf/scripts/start-waf.ps1 -Build
#   powershell -File Nerve-waf/Nerve-waf/scripts/start-waf.ps1 -PortForward   # 需管理员
#   powershell -File Nerve-waf/Nerve-waf/scripts/start-waf.ps1 -PortForwardClear

param(
    [switch]$Build,
    [switch]$Stop,
    [switch]$NoConsole,
    [switch]$PortForward,
    [switch]$PortForwardClear,
    [Alias("PortForwardRemove")]
    [switch]$PortForwardRemoveAlias,
    [string]$WslIp = ""
)

function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Invoke-WafPortForward {
    param(
        [switch]$Clear,
        [string]$WslIpArg = ""
    )
    $forwardScript = Join-Path $PSScriptRoot "wsl-port-forward.ps1"
    if (-not (Test-Path -LiteralPath $forwardScript)) {
        throw "missing $forwardScript"
    }
    if (-not (Test-IsAdmin)) {
        Write-Host "[start-waf] port forward requires Administrator — elevating …" -ForegroundColor Yellow
        $argList = @("-ExecutionPolicy", "Bypass", "-File", $forwardScript)
        if ($Clear) { $argList += "-Clear" }
        if ($WslIpArg) { $argList += "-WslIp"; $argList += $WslIpArg }
        Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $argList -Wait
        return
    }
    if ($Clear) {
        if ($WslIpArg) {
            & $forwardScript -Clear -WslIp $WslIpArg
        } else {
            & $forwardScript -Clear
        }
    } elseif ($WslIpArg) {
        & $forwardScript -WslIp $WslIpArg
    } else {
        & $forwardScript
    }
}

$portForwardClear = $PortForwardClear -or $PortForwardRemoveAlias

if ($portForwardClear) {
    Invoke-WafPortForward -Clear -WslIpArg $WslIp
    exit 0
}

if ($PortForward) {
    Invoke-WafPortForward -WslIpArg $WslIp
}

$ProductRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$drive = $ProductRoot.Substring(0, 1).ToLower()
$rest = $ProductRoot.Substring(3) -replace '\\', '/'
$UnixRoot = "/mnt/$drive$rest"

$wsl = "C:\Windows\System32\wsl.exe"
if (-not (Test-Path $wsl)) {
    throw "WSL required. Install WSL then run: bash scripts/start-waf.sh"
}

$argsList = @()
if ($Build) { $argsList += "--build" }
if ($Stop) { $argsList += "--stop" }
if ($NoConsole) { $argsList += "--no-console" }
$argStr = ($argsList -join " ")

$cmd = "cd '$UnixRoot' && bash scripts/start-waf.sh $argStr"
Write-Host ">> WSL: $cmd"
Write-Host ""
Write-Host "Windows/LAN (after port forward):"
Write-Host "  Controller http://localhost:9000/api/health"
Write-Host "  Agent      http://localhost:9080/health"
Write-Host "  Console    http://localhost:9090/"
Write-Host "  Map ports (Admin): powershell -File scripts/wsl-port-forward.ps1"
Write-Host ""
& $wsl -e bash -lc $cmd
