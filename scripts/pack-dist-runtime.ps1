#Requires -Version 5.1
param(
    [switch]$Build
)

$ErrorActionPreference = "Stop"

# Windows：从 WSL 编译产物或已有 Linux 二进制打包到 dist/runtime/
#
#   powershell -File Nerve-waf/scripts/pack-dist-runtime.ps1
#   powershell -File Nerve-waf/scripts/pack-dist-runtime.ps1 -Build   # 调 WSL cargo build

$WafRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$RepoRoot = (Resolve-Path (Join-Path $WafRoot "..")).Path
$Out = Join-Path $WafRoot "dist\runtime"
$BinApi = Join-Path $WafRoot "target\release\nerve-waf-api"
$BinEdge = Join-Path $WafRoot "target\release\nerve-waf-edge"
$PackSrc = Join-Path $RepoRoot "Nerve-waf-native\packs\baseline"

if ($Build) {
    $wsl = Get-Command wsl -ErrorAction SilentlyContinue
    if (-not $wsl) { throw "wsl not found — install WSL or build manually in Linux" }
    $drive = $WafRoot.Substring(0, 1).ToLower()
    $rest = $WafRoot.Substring(3) -replace '\\', '/'
    $unix = "/mnt/$drive$rest"
    wsl bash -lc "cd '$unix' && cargo build --release -p nerve-waf-api -p nerve-waf-edge"
}

function Test-LinuxElf([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }
    $b = [IO.File]::ReadAllBytes($Path)
    return ($b.Length -ge 4 -and $b[0] -eq 0x7f -and $b[1] -eq 0x45 -and $b[2] -eq 0x4c -and $b[3] -eq 0x46)
}

foreach ($p in @($BinApi, $BinEdge)) {
    if (-not (Test-Path $p)) {
        throw "missing $p — run WSL: cargo build --release -p nerve-waf-api -p nerve-waf-edge"
    }
    if (-not (Test-LinuxElf $p)) {
        throw "not Linux ELF: $p — rebuild in WSL, not Windows host"
    }
}

if (-not (Test-Path $PackSrc)) {
    throw "missing baseline pack: $PackSrc"
}

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
$null = New-Item -ItemType Directory -Force -Path (Join-Path $Out "baseline")

Copy-Item -Force $BinApi (Join-Path $Out "nerve-waf-api")
Copy-Item -Force $BinEdge (Join-Path $Out "nerve-waf-edge")
Copy-Item -Recurse -Force (Join-Path $PackSrc "*") (Join-Path $Out "baseline")

$manifest = @(
    "packed_at=$(Get-Date -Format o)"
    "nerve-waf-api=$((Get-FileHash (Join-Path $Out 'nerve-waf-api') -Algorithm SHA256).Hash.ToLower())"
    "nerve-waf-edge=$((Get-FileHash (Join-Path $Out 'nerve-waf-edge') -Algorithm SHA256).Hash.ToLower())"
    "baseline=$((Get-ChildItem (Join-Path $Out 'baseline') -Recurse -File).Count) files"
)
$manifest | Set-Content -Encoding UTF8 (Join-Path $Out "MANIFEST.txt")

Write-Host ">> done: $Out"
Get-ChildItem $Out | Format-Table Name, Length, LastWriteTime
Write-Host "copy to test: dist\runtime\ + dist\nerve-waf-api-*.tar + deploy\mars-test\"
