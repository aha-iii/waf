#Requires -Version 5.1
$ErrorActionPreference = "Stop"
$env:PATH = "C:\Program Files\Docker\Docker\resources\bin;" + $env:PATH
$Deploy = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Deploy
$Tar = Join-Path $Deploy "..\dist\nerve-waf-api-1.0.0.tar"
try {
  docker image inspect nerve/nerve-waf-api:1.0.0 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "not found" }
} catch {
  if (-not (Test-Path $Tar)) { throw "missing $Tar" }
  docker load -i $Tar
}
docker compose up -d
Write-Host "API/Console: http://127.0.0.1:8781/api/health"
Write-Host "Edge:        http://127.0.0.1:8080/health"
