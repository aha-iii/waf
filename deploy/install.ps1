# Nerve WAF 一键安装 — Windows 入口
#
# Windows 上 Docker Compose 的构建上下文与挂载路径在 WSL 下更稳，故本脚本转调
# WSL 中的 deploy/install.sh，与仓库既有的 scripts/up.ps1 保持一致。
#
#   powershell -ExecutionPolicy Bypass -File install.ps1
#   powershell -ExecutionPolicy Bypass -File install.ps1 --standalone
#   powershell -ExecutionPolicy Bypass -File install.ps1 --down
#
# 前置：WSL2 + 已安装 Docker（可在 WSL 内执行 `docker info`）
# 提示：若改用 Docker Desktop 原生 Compose，请直接在 WSL 里执行 bash install.sh

param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = "Stop"

$wsl = "C:\Windows\System32\wsl.exe"
if (-not (Test-Path $wsl)) {
  Write-Error "未找到 wsl.exe。请先安装 WSL2：wsl --install"
}

$deployWin = (Resolve-Path (Join-Path $PSScriptRoot ".")).Path
$drive = $deployWin.Substring(0, 1).ToLower()
$rest = $deployWin.Substring(2).Replace("\", "/")
$wslDeploy = "/mnt/$drive$rest"

$argStr = ""
if ($Args.Count -gt 0) {
  $argStr = ($Args | ForEach-Object { "'$_'" }) -join " "
}

Write-Host "────────────────────────────────────────────────"
Write-Host " Nerve WAF 一键安装（经 WSL）"
Write-Host " 目录：$wslDeploy"
Write-Host "────────────────────────────────────────────────"

& $wsl -e bash -lc "cd '$wslDeploy' && bash install.sh $argStr"
$code = $LASTEXITCODE

if ($code -ne 0) {
  Write-Host ""
  Write-Host "✗ 安装失败（exit=$code）。排查："
  Write-Host "   wsl -e bash -lc `"cd '$wslDeploy' && docker compose logs --tail=80`""
}

exit $code
