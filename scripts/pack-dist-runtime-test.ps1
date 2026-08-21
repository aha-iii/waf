#Requires -Version 5.1
param(
    [switch]$BuildFrontend
)

$ErrorActionPreference = "Stop"
$WafRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$RepoRoot = (Resolve-Path (Join-Path $WafRoot "..")).Path
$MonoRoot = (Resolve-Path (Join-Path $RepoRoot "..")).Path
$Out = Join-Path $WafRoot "dist\runtime"
$BinApi = Join-Path $WafRoot "target\release\nerve-waf-api"
$BinConsole = Join-Path $WafRoot "target\release\nerve-waf-console"
$BinEdge = Join-Path $WafRoot "target\release\nerve-waf-edge"
$BinAlert = Join-Path $MonoRoot "Nerve-alert\target\release\nerve-alert-api"
$PackSrc = Join-Path $RepoRoot "Nerve-antibody\packs\baseline"
$ConsoleDir = Join-Path $WafRoot "apps\console-react"
$WebsiteDir = Join-Path $MonoRoot "Nerve-website"
$ExcludeRules = @("ssrf-internal-ip", "ssrf-loopback")

function Test-LinuxElf([string]$Path) {
    if (-not (Test-Path $Path)) { return $false }
    $b = [IO.File]::ReadAllBytes($Path)
    return ($b.Length -ge 4 -and $b[0] -eq 0x7f -and $b[1] -eq 0x45 -and $b[2] -eq 0x4c -and $b[3] -eq 0x46)
}

if ($BuildFrontend) {
    Write-Host ">> npm build console-react"
    Push-Location $ConsoleDir
    npm install
    npx vite build
    Pop-Location
    Write-Host ">> npm build Nerve-website"
    Push-Location $WebsiteDir
    npm ci
    npm run build
    Pop-Location
}

foreach ($p in @($BinApi, $BinConsole, $BinEdge, $BinAlert)) {
    if (-not (Test-Path $p)) { throw "missing $p — build in Linux/WSL first" }
    if (-not (Test-LinuxElf $p)) { throw "not Linux ELF: $p" }
}

$consoleDist = Join-Path $ConsoleDir "dist\index.html"
$websiteDist = Join-Path $WebsiteDir "dist\index.html"
if (-not (Test-Path $consoleDist)) { throw "missing console dist" }
if (-not (Test-Path $websiteDist)) { throw "missing website dist" }
if (-not (Test-Path $PackSrc)) { throw "missing baseline: $PackSrc" }

if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
New-Item -ItemType Directory -Force -Path (Join-Path $Out "baseline"), (Join-Path $Out "console"), (Join-Path $Out "official") | Out-Null

Copy-Item -Force $BinApi (Join-Path $Out "nerve-waf-api")
Copy-Item -Force $BinConsole (Join-Path $Out "nerve-waf-console")
Copy-Item -Force $BinEdge (Join-Path $Out "nerve-waf-edge")
Copy-Item -Force $BinAlert (Join-Path $Out "nerve-alert-api")
Copy-Item -Recurse -Force (Join-Path $PackSrc "*") (Join-Path $Out "baseline")
Copy-Item -Recurse -Force (Join-Path $ConsoleDir "dist\*") (Join-Path $Out "console")
Copy-Item -Recurse -Force (Join-Path $WebsiteDir "dist\*") (Join-Path $Out "official")

foreach ($id in $ExcludeRules) {
    Get-ChildItem (Join-Path $Out "baseline\rules") -Recurse -File | Where-Object { $_.BaseName -eq $id } | ForEach-Object {
        Write-Host ">> test pack: removed $($_.FullName)"
        Remove-Item -Force $_.FullName
    }
}

$samples = Join-Path $Out "baseline\samples\05-rfi-ssrf.yaml"
if (Test-Path $samples) {
    $text = Get-Content -Raw -Encoding UTF8 $samples
    $blocks = $text -split "`n- id:"
    $kept = @($blocks[0])
    $removed = 0
    foreach ($block in $blocks[1..($blocks.Length - 1)]) {
        $chunk = "- id:" + $block
        $skip = $false
        foreach ($id in $ExcludeRules) {
            if ($chunk -match "rule_id:\s*$id") { $skip = $true; $removed++; break }
        }
        if (-not $skip) { $kept += $block }
    }
    ($kept -join "`n- id:") | Set-Content -Encoding UTF8 $samples
    Write-Host ">> test pack: pruned $removed sample(s)"
}

@(
    "packed_at=$(Get-Date -Format o)"
    "pack_profile=mars-test"
    "excluded_rules=$($ExcludeRules -join ',')"
    "nerve-waf-api=$((Get-FileHash (Join-Path $Out 'nerve-waf-api') -Algorithm SHA256).Hash.ToLower())"
    "nerve-waf-edge=$((Get-FileHash (Join-Path $Out 'nerve-waf-edge') -Algorithm SHA256).Hash.ToLower())"
    "nerve-alert-api=$((Get-FileHash (Join-Path $Out 'nerve-alert-api') -Algorithm SHA256).Hash.ToLower())"
) | Set-Content -Encoding UTF8 (Join-Path $Out "MANIFEST.txt")

Write-Host ">> done: $Out"
