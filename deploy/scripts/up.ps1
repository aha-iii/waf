# Windows 入口：转调 WSL bash scripts/up.sh
param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"
$wsl = "C:\Windows\System32\wsl.exe"
$deployWin = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$drive = $deployWin.Substring(0, 1).ToLower()
$rest = $deployWin.Substring(2).Replace("\", "/")
$wslDeploy = "/mnt/$drive$rest"
$argStr = ($Args | ForEach-Object { "'$_'" }) -join " "
& $wsl -e bash -lc "cd '$wslDeploy' && bash scripts/up.sh $argStr"
