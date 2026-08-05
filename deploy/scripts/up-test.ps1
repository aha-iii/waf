#Requires -Version 5.1
$ErrorActionPreference = "Stop"

$Deploy = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Deploy

$env:PATH = "C:\Program Files\Docker\Docker\resources\bin;" + $env:PATH

if (-not (Test-Path ".env")) {
  Copy-Item ".env.example" ".env"
  Write-Host "[up-test] created .env from .env.example"
}

$Tag = if ($env:TAG) { $env:TAG } else { "1.0.0" }
$Image = "nerve/nerve-waf-api:$Tag"

$imgExists = $false
try {
  docker image inspect $Image 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { $imgExists = $trun }
} catch {}

if (-not $imgExists) {
  $tar = Join-Path $Deploy "..\dist\nerve-waf-api-$Tag.tar"
  if (Test-Path $tar) {
    Write-Host "[up-test] loading image from $tar"
    docker load -i $tar
  } else {
    Write-Host "[up-test] image $Image not found — compose will build or run build-image.ps1"
  }
}

$usePg = ($args -contains "postgres") -or ($args -contains "--profile")
if ($usePg) {
  $envContent = Get-Content ".env" -Raw -ErrorAction SilentlyContinue
  if ($envContent -notmatch "DATABASE_URL=postgres://") {
    Add-Content ".env" "DATABASE_URL=postgres://nerve:nerve@postgres:5432/nerve"
  }
  docker compose --profile postgres up -d
} else {
  docker compose up -d
}

$port = if ($env:PORT) { $env:PORT } else { "8781" }
Write-Host "[up-test] WAF API: http://127.0.0.1:${port}/api/health"
Write-Host "[up-test] expert:  http://127.0.0.1:${port}/api/expert/health"
