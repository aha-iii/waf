#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$WafRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$RepoRoot = Resolve-Path (Join-Path $WafRoot "..")
$Tag = if ($env:TAG) { $env:TAG } else { "1.0.0" }
$Image = if ($env:IMAGE) { $env:IMAGE } else { "nerve/nerve-waf-api:$Tag" }
$Out = if ($env:OUT) { $env:OUT } else { Join-Path $WafRoot "dist\nerve-waf-api-$Tag.tar" }
$UsePrebuilt = ($env:USE_PREBUILT -eq "true")
$Platform = if ($env:DOCKER_PLATFORM) { $env:DOCKER_PLATFORM } else { "linux/amd64" }
$ViteTz = if ($env:VITE_CONSOLE_TIME_ZONE) { $env:VITE_CONSOLE_TIME_ZONE } else { "Asia/Shanghai" }
$BuildArgs = @("--build-arg", "VITE_CONSOLE_TIME_ZONE=$ViteTz")

$Docker = "docker"
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
  $candidate = "C:\Program Files\Docker\Docker\resources\bin\docker.exe"
  if (Test-Path $candidate) { $Docker = $candidate } else { throw "docker not found" }
}
$env:PATH = "C:\Program Files\Docker\Docker\resources\bin;" + $env:PATH

$null = New-Item -ItemType Directory -Force -Path (Split-Path $Out)

if ($UsePrebuilt) {
  $binApiUnix = Join-Path $WafRoot "target\release\nerve-waf-api"
  $binApiWin = Join-Path $WafRoot "target\release\nerve-waf-api.exe"
  $binEdgeUnix = Join-Path $WafRoot "target\release\nerve-waf-edge"
  $binEdgeWin = Join-Path $WafRoot "target\release\nerve-waf-edge.exe"
  if (-not (Test-Path $binApiUnix) -and -not (Test-Path $binApiWin)) {
    throw "missing prebuilt nerve-waf-api — build on Linux/WSL: cargo build --release -p nerve-waf-api -p nerve-waf-edge"
  }
  if (-not (Test-Path $binEdgeUnix) -and -not (Test-Path $binEdgeWin)) {
    throw "missing prebuilt nerve-waf-edge — build on Linux/WSL: cargo build --release -p nerve-waf-api -p nerve-waf-edge"
  }
  $artifactDir = Join-Path $WafRoot "deploy\docker\artifacts"
  $artifactApi = Join-Path $artifactDir "nerve-waf-api"
  $artifactEdge = Join-Path $artifactDir "nerve-waf-edge"
  $null = New-Item -ItemType Directory -Force -Path $artifactDir
  if (Test-Path $binApiUnix) { Copy-Item -Force $binApiUnix $artifactApi } else { Copy-Item -Force $binApiWin $artifactApi }
  if (Test-Path $binEdgeUnix) { Copy-Item -Force $binEdgeUnix $artifactEdge } else { Copy-Item -Force $binEdgeWin $artifactEdge }
  & $Docker build --platform $Platform @BuildArgs `
    -f (Join-Path $WafRoot "deploy\docker\Dockerfile.prebuilt") `
    -t $Image $RepoRoot
  if ($LASTEXITCODE -ne 0) { throw "docker build failed (exit $LASTEXITCODE)" }
} else {
  & $Docker build --platform $Platform @BuildArgs `
    -f (Join-Path $WafRoot "deploy\docker\Dockerfile") `
    -t $Image $RepoRoot
  if ($LASTEXITCODE -ne 0) { throw "docker build failed (exit $LASTEXITCODE)" }
}

& $Docker save -o $Out $Image
if ($LASTEXITCODE -ne 0) { throw "docker save failed (exit $LASTEXITCODE)" }
Write-Host "saved: $Out"
Write-Host "load: docker load -i $Out"
