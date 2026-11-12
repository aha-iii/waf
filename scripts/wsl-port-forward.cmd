@echo off
setlocal
REM WAF 端口映射（9000/9080/9090 → WSL）。需管理员 CMD/PowerShell。
REM   d:\workspace\waf\Nerve-waf\Nerve-waf\scripts\wsl-port-forward.cmd
REM   wsl-port-forward.cmd -Clear

set "PS1=%~dp0wsl-port-forward.ps1"
if not exist "%PS1%" (
  echo missing: %PS1%
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*
exit /b %ERRORLEVEL%
