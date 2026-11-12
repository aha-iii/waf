@echo off
setlocal
REM 启动 WAF 三进程（WSL）+ 可选端口映射
REM   start-waf.cmd -Build
REM   start-waf.cmd -PortForward -Build

set "PS1=%~dp0start-waf.ps1"
if not exist "%PS1%" (
  echo missing: %PS1%
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1%" %*
exit /b %ERRORLEVEL%
