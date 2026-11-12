#!/usr/bin/env bash
#
# Nerve WAF — one-command installer (Docker Compose)
# Copyright (c) 2024-2026 Nerve Team · MIT License
#
# Usage:
#   bash install.sh [OPTIONS]
#
# Options:
#   --standalone        Controller + Agent only (no Console UI container)
#   --full              Require Console UI source; fail if absent
#   --no-build          Reuse existing images, skip rebuild
#   --skip-smoke        Skip post-install smoke test
#   --port N            Controller port (default: PORT in .env, else 9000)
#   --agent-port N       Agent port (default: WAF_AGENT_PORT in .env, else 9080)
#   --timeout N         Health-check timeout in seconds (default: 180)
#   --force-env         Regenerate .env from .env.example (backs up the old one)
#   --dry-run           Print every command without executing it
#   --verbose           Remove orphan containers before start
#   -y, --yes           Assume yes; never prompt
#   --down              Stop and remove containers (keeps volumes/data)
#   --uninstall         Stop, remove containers and named volumes
#   --purge             With --uninstall: also delete ./data and ./logs
#   --version           Print installer version
#   -h, --help          Show this help
#
# Requires: Docker Engine 24+ with the Compose v2 plugin (`docker compose`).
# Docs:     ../docs/installation.md · START.md · ../docs/runbooks/PRODUCTION.md
#
# Exit codes: 0 success · 1 runtime failure · 2 usage error · 3 preflight failure

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -Eeuo pipefail

readonly PROG="install.sh"
readonly INSTALLER_VERSION="1.1.0"
readonly MIN_DOCKER_MAJOR=24
readonly MIN_DISK_GB=5

DEPLOY="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$DEPLOY/.." && pwd)"
cd "$DEPLOY"

LOG_FILE="${NERVE_INSTALL_LOG:-${TMPDIR:-/tmp}/nerve-waf-install-$(date +%Y%m%d-%H%M%S).log}"
if ! : >"$LOG_FILE" 2>/dev/null; then
  LOG_FILE="/dev/null"
fi

COMPOSE=(docker compose)
FILES=(-f docker-compose.yml)
MODE="auto"
BUILD=1
SMOKE=1
ACTION="install"
DRY_RUN=0
VERBOSE=0
ASSUME_YES=0
FORCE_ENV=0
PURGE=0
HAS_CURL=0
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-180}"
PORT_OVERRIDE=""
AGENT_PORT_OVERRIDE=""

# ------------------------------------------------------------------ output
if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]] && [[ "${TERM:-dumb}" != "dumb" ]]; then
  C_RESET=$'\033[0m' C_BOLD=$'\033[1m' C_DIM=$'\033[2m'
  C_RED=$'\033[31m' C_GREEN=$'\033[32m' C_YELLOW=$'\033[33m' C_CYAN=$'\033[36m'
else
  C_RESET="" C_BOLD="" C_DIM="" C_RED="" C_GREEN="" C_YELLOW="" C_CYAN=""
fi

_emit() {
  local line="$1"
  printf '%s\n' "$line"
  if [[ "$LOG_FILE" != "/dev/null" ]]; then
    printf '%s\n' "$line" >>"$LOG_FILE" 2>/dev/null || true
  fi
}

log() { _emit "${C_CYAN}[install]${C_RESET} $*"; }
ok() { _emit "${C_GREEN}[install] ✓${C_RESET} $*"; }
warn() { _emit "${C_YELLOW}[install] !${C_RESET} $*"; }
err() { _emit "${C_RED}[install] ✗${C_RESET} $*" >&2; }
step() {
  _emit ""
  _emit "${C_BOLD}▶ $*${C_RESET}"
}

usage() {
  sed -n '/^# Usage:/,/^# Exit codes:/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'
}

die() {
  local code="${2:-1}"
  err "$1"
  exit "$code"
}

on_error() {
  local code=$? line=$1 cmd=$2
  if [[ $code -eq 0 ]]; then
    return 0
  fi
  _emit ""
  err "安装中断（退出码 ${code}，第 ${line} 行）"
  _emit "  失败命令：${cmd}"
  _emit "  完整日志：${LOG_FILE}"
  _emit "  排查建议：docker compose ${FILES[*]} logs --tail=100"
  exit "$code"
}
trap 'on_error $LINENO "$BASH_COMMAND"' ERR

run() {
  if ((DRY_RUN)); then
    _emit "${C_DIM}  [dry-run] $*${C_RESET}"
    return 0
  fi
  "$@"
}

# ------------------------------------------------------------------ args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --standalone)
      MODE="standalone"
      shift
      ;;
    --full)
      MODE="full"
      shift
      ;;
    --no-build)
      BUILD=0
      shift
      ;;
    --skip-smoke)
      SMOKE=0
      shift
      ;;
    --force-env)
      FORCE_ENV=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --verbose)
      VERBOSE=1
      shift
      ;;
    -y | --yes)
      ASSUME_YES=1
      shift
      ;;
    --down)
      ACTION="down"
      shift
      ;;
    --uninstall)
      ACTION="uninstall"
      shift
      ;;
    --purge)
      ACTION="uninstall"
      PURGE=1
      shift
      ;;
    --port)
      if [[ $# -lt 2 ]]; then
        die "--port 需要一个端口号" 2
      fi
      PORT_OVERRIDE="$2"
      shift 2
      ;;
    --port=*)
      PORT_OVERRIDE="${1#*=}"
      shift
      ;;
    --agent-port)
      if [[ $# -lt 2 ]]; then
        die "--agent-port 需要一个端口号" 2
      fi
      AGENT_PORT_OVERRIDE="$2"
      shift 2
      ;;
    --agent-port=*)
      AGENT_PORT_OVERRIDE="${1#*=}"
      shift
      ;;
    --timeout)
      if [[ $# -lt 2 ]]; then
        die "--timeout 需要一个秒数" 2
      fi
      HEALTH_TIMEOUT="$2"
      shift 2
      ;;
    --timeout=*)
      HEALTH_TIMEOUT="${1#*=}"
      shift
      ;;
    --version)
      printf '%s %s\n' "$PROG" "$INSTALLER_VERSION"
      exit 0
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      err "未知参数：$1"
      _emit "  用 --help 查看全部选项"
      exit 2
      ;;
  esac
done

if [[ ! "$HEALTH_TIMEOUT" =~ ^[0-9]+$ ]]; then
  die "--timeout 必须是整数秒" 2
fi
if [[ -n "$PORT_OVERRIDE" && ! "$PORT_OVERRIDE" =~ ^[0-9]+$ ]]; then
  die "--port 必须是整数" 2
fi
if [[ -n "$AGENT_PORT_OVERRIDE" && ! "$AGENT_PORT_OVERRIDE" =~ ^[0-9]+$ ]]; then
  die "--agent-port 必须是整数" 2
fi

# ------------------------------------------------------------------ preflight
have() { command -v "$1" >/dev/null 2>&1; }

# Port probe via netstat/ss. Avoids /dev/tcp inside a subshell: with `set -E`
# the ERR trap is inherited by subshells and an expected connect failure would
# abort the whole installer.
port_busy() {
  local p="$1"
  if have ss; then
    if ss -ltn 2>/dev/null | awk 'NR>1{print $4}' | grep -qE "[:.]${p}$"; then
      return 0
    fi
    return 1
  fi
  if have netstat; then
    if netstat -ltn 2>/dev/null | awk 'NR>2{print $4}' | grep -qE "[:.]${p}$"; then
      return 0
    fi
    # Windows: `netstat -ano` lists the LISTENING state explicitly
    if netstat -ano 2>/dev/null | grep -qE "[:.]${p}[[:space:]]+.*LISTENING"; then
      return 0
    fi
    return 1
  fi
  return 1
}

check_docker() {
  if ! have docker; then
    die "未找到 docker。安装：https://docs.docker.com/engine/install/" 3
  fi

  if ! docker info >/dev/null 2>&1; then
    die "Docker 守护进程不可用。
  Linux：sudo systemctl start docker
  macOS/Windows：启动 Docker Desktop 并等待其就绪
  权限：当前用户需在 docker 组内（sudo usermod -aG docker \$USER 后重新登录）" 3
  fi

  local ver major
  ver="$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo "0.0.0")"
  major="${ver%%.*}"
  if [[ "$major" =~ ^[0-9]+$ ]] && ((major < MIN_DOCKER_MAJOR)); then
    die "Docker 服务端版本 ${ver} 过旧，需要 ${MIN_DOCKER_MAJOR}+" 3
  fi

  if ! docker compose version >/dev/null 2>&1; then
    if have docker-compose; then
      die "检测到旧版 docker-compose v1。本仓库 compose 文件需要 Compose v2 插件（\`docker compose\`）。
  安装：https://docs.docker.com/compose/install/" 3
    fi
    die "未找到 Docker Compose v2 插件。安装：https://docs.docker.com/compose/install/" 3
  fi

  if have curl; then
    HAS_CURL=1
  fi

  ok "docker ${ver} · compose $(docker compose version --short 2>/dev/null || echo v2)"
}

check_disk() {
  local need_kb=$((MIN_DISK_GB * 1024 * 1024))
  local free_kb free_gb
  free_kb="$(df -Pk "$DEPLOY" 2>/dev/null | awk 'NR==2{print $4}')"
  if [[ ! "$free_kb" =~ ^[0-9]+$ ]]; then
    return 0
  fi
  free_gb=$((free_kb / 1024 / 1024))
  if ((free_kb < need_kb)); then
    warn "部署目录所在磁盘仅剩 ${free_gb}GB（建议 ≥${MIN_DISK_GB}GB 供镜像与数据使用）"
    warn "先释放空间：docker system prune -af"
  else
    ok "磁盘剩余 ${free_gb}GB"
  fi
}

check_ports() {
  local busy=() port
  for port in "$CONTROLLER_PORT" "$AGENT_PORT"; do
    if [[ -z "$port" ]]; then
      continue
    fi
    if port_busy "$port"; then
      busy+=("$port")
    fi
  done

  if ((${#busy[@]} == 0)); then
    ok "端口可用：${CONTROLLER_PORT} / ${AGENT_PORT}"
    return 0
  fi

  local list
  list="$(printf '%s ' "${busy[@]}")"
  die "端口被占用：${list% }
  换端口：bash install.sh --port 9100 --agent-port 9180
  查占用：netstat -ano | findstr :${busy[0]}   (Windows)
          lsof -iTCP:${busy[0]} -sTCP:LISTEN    (Linux/macOS)" 3
}

# ------------------------------------------------------------------ .env
prepare_env() {
  if [[ -f .env ]] && ((!FORCE_ENV)); then
    ok "复用已有 .env（重置：--force-env）"
    return 0
  fi
  if [[ -f .env ]]; then
    local bak=".env.bak.$(date +%Y%m%d-%H%M%S)"
    run cp .env "$bak"
    ok "已备份旧 .env → ${bak}"
  fi
  if [[ ! -f .env.example ]]; then
    die "缺少 .env.example，仓库不完整" 3
  fi
  run cp .env.example .env
  ok "已从 .env.example 生成 .env"
}

envval() {
  local v
  v="$(sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*//p" .env 2>/dev/null | tail -1 | sed 's/[[:space:]]*$//')"
  v="${v%\"}"
  v="${v#\"}"
  printf '%s' "${v:-$2}"
}

# ------------------------------------------------------------------ UI detection
resolve_ui() {
  if [[ "$MODE" == "standalone" ]]; then
    ok "按 --standalone 运行：仅 Controller + Agent"
    FILES+=(-f docker-compose.standalone.yml)
    return 1
  fi

  local cands=() d
  if [[ -n "${NERVE_UI_DIR:-}" ]]; then
    cands+=("$NERVE_UI_DIR")
  fi
  cands+=("$REPO_ROOT/../Nerve-ui" "$REPO_ROOT/../../Nerve-ui")

  for d in "${cands[@]}"; do
    if [[ -f "$d/package.json" ]]; then
      ok "找到 Console UI 源码：$d"
      return 0
    fi
  done

  if [[ "$MODE" == "full" ]]; then
    die "--full 要求 Console UI 源码，但未找到。
  指定路径：NERVE_UI_DIR=/path/to/Nerve-ui bash install.sh --full
  或改用：bash install.sh --standalone" 3
  fi

  warn "未找到 Console UI 源码，自动降级为 standalone（仅 Controller + Agent）"
  _emit "  ${C_DIM}需要 Web 控制台：NERVE_UI_DIR=/path/to/Nerve-ui bash install.sh${C_RESET}"
  FILES+=(-f docker-compose.standalone.yml)
  return 1
}

# ------------------------------------------------------------------ health
wait_http() {
  local url="$1" name="$2" limit="${3:-$HEALTH_TIMEOUT}"
  local waited=0

  if ((HAS_CURL == 0)); then
    warn "未安装 curl，跳过 ${name} HTTP 健康探测"
    return 0
  fi

  log "等待 ${name} 就绪：${url}（上限 ${limit}s）"
  while ((waited < limit)); do
    if curl -fsS --max-time 3 -o /dev/null "$url" 2>/dev/null; then
      ok "${name} 就绪（${waited}s）"
      return 0
    fi
    sleep 2
    waited=$((waited + 2))
    if ((waited % 20 == 0)); then
      _emit "  ${C_DIM}...已等待 ${waited}s${C_RESET}"
    fi
  done

  err "${name} 在 ${limit}s 内未就绪"
  return 1
}

show_failure_context() {
  _emit ""
  warn "最近 60 行容器日志："
  if ((DRY_RUN)); then
    _emit "${C_DIM}  [dry-run] skip logs${C_RESET}"
  else
    "${COMPOSE[@]}" "${FILES[@]}" logs --tail=60 2>&1 | sed 's/^/  /' >&2 || true
  fi
  _emit ""
  warn "容器已保留以便排查。修复后重试，或 bash install.sh --down 清理"
}

compose_up() {
  local args=(up -d)
  if ((BUILD)); then
    args+=(--build)
  fi
  if ((VERBOSE)); then
    args+=(--remove-orphans)
  fi
  log "执行：${COMPOSE[*]} ${FILES[*]} ${args[*]}"
  if ((DRY_RUN)); then
    run "${COMPOSE[@]}" "${FILES[@]}" "${args[@]}"
    return 0
  fi
  COMPOSE_PROGRESS=plain "${COMPOSE[@]}" "${FILES[@]}" "${args[@]}"
}

confirm() {
  if ((ASSUME_YES)); then
    return 0
  fi
  local reply
  if ! read -r -p "$1 [y/N] " reply </dev/tty 2>/dev/null; then
    return 1
  fi
  [[ "$reply" =~ ^[Yy]$ ]]
}

# ------------------------------------------------------------------ teardown
do_down() {
  check_docker
  step "停止并移除容器"
  run "${COMPOSE[@]}" -f docker-compose.yml -f docker-compose.standalone.yml down --remove-orphans 2>/dev/null || true
  run "${COMPOSE[@]}" -f docker-compose.yml down --remove-orphans
  ok "已停止（数据卷保留）"
  _emit "  重新启动：bash install.sh"
}

do_uninstall() {
  check_docker
  step "卸载 Nerve WAF"
  warn "将移除容器与命名数据卷；数据库数据不可恢复。"
  if ! confirm "确认卸载？"; then
    log "已取消"
    exit 0
  fi

  run "${COMPOSE[@]}" -f docker-compose.yml -f docker-compose.standalone.yml down -v --remove-orphans 2>/dev/null || true
  run "${COMPOSE[@]}" -f docker-compose.yml down -v --remove-orphans
  ok "容器与数据卷已移除"

  if ((PURGE)); then
    warn "额外删除本地 data/ 与 logs/ 目录"
    if confirm "确认删除本地数据目录？"; then
      run rm -rf "$DEPLOY/data" "$DEPLOY/logs"
      ok "已删除本地数据目录"
    else
      log "已保留 data/ 与 logs/"
    fi
  fi

  _emit "  .env 已保留（含你的配置）。完全清理：rm -f .env"
}

# ------------------------------------------------------------------ main
if [[ "$ACTION" == "down" ]]; then
  do_down
  exit 0
fi

if [[ "$ACTION" == "uninstall" ]]; then
  do_uninstall
  exit 0
fi

_emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
_emit "${C_BOLD} Nerve WAF 安装${C_RESET}  ${C_DIM}v${INSTALLER_VERSION}${C_RESET}"
_emit " 目录：${DEPLOY}"
_emit " 日志：${LOG_FILE}"
if ((DRY_RUN)); then
  _emit " ${C_YELLOW}模式：dry-run（不会产生任何变更）${C_RESET}"
fi
_emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"

step "1/6 环境预检"
check_docker

step "2/6 配置准备"
prepare_env

if [[ -n "$PORT_OVERRIDE" ]]; then
  export PORT="$PORT_OVERRIDE"
  ok "Controller 端口覆盖为 ${PORT}"
fi
if [[ -n "$AGENT_PORT_OVERRIDE" ]]; then
  export WAF_AGENT_PORT="$AGENT_PORT_OVERRIDE"
  ok "Agent 端口覆盖为 ${WAF_AGENT_PORT}"
fi

CONTROLLER_PORT="$(envval PORT 9000)"
AGENT_PORT="$(envval WAF_AGENT_PORT 9080)"
CONSOLE_PORT="$(envval CONSOLE_PORT 9090)"

# CLI flags win over .env values
if [[ -n "$PORT_OVERRIDE" ]]; then
  CONTROLLER_PORT="$PORT_OVERRIDE"
fi
if [[ -n "$AGENT_PORT_OVERRIDE" ]]; then
  AGENT_PORT="$AGENT_PORT_OVERRIDE"
fi

check_disk
check_ports

step "3/6 组件选择"
HAS_UI=1
if ! resolve_ui; then
  HAS_UI=0
fi

step "4/6 构建并启动"
compose_up

if ((DRY_RUN)); then
  _emit ""
  _emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
  _emit " ${C_GREEN}dry-run 完成${C_RESET}：以上命令均未执行，环境无任何变更。"
  _emit " 去掉 --dry-run 即开始真实安装。"
  _emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
  exit 0
fi

step "5/6 健康检查"
FAILED=0
if ! wait_http "http://127.0.0.1:${CONTROLLER_PORT}/api/health" "Controller"; then
  FAILED=1
fi
if ! wait_http "http://127.0.0.1:${AGENT_PORT}/health" "Agent"; then
  FAILED=1
fi
if ((HAS_UI)); then
  if ! wait_http "http://127.0.0.1:${CONSOLE_PORT}/" "Console" "${CONSOLE_TIMEOUT:-60}"; then
    warn "Console 未就绪（不影响 API 使用）"
  fi
fi

step "6/6 冒烟验证"
if ((SMOKE)); then
  if [[ -x "$REPO_ROOT/scripts/smoke-test.sh" ]]; then
    if ((DRY_RUN)); then
      _emit "${C_DIM}  [dry-run] bash scripts/smoke-test.sh${C_RESET}"
    else
      if ! CONTROLLER_URL="http://127.0.0.1:${CONTROLLER_PORT}" \
        AGENT_URL="http://127.0.0.1:${AGENT_PORT}" \
        CONSOLE_URL="http://127.0.0.1:${CONSOLE_PORT}" \
        bash "$REPO_ROOT/scripts/smoke-test.sh"; then
        FAILED=1
      fi
    fi
  else
    warn "未找到 scripts/smoke-test.sh，跳过"
  fi
else
  log "已跳过（--skip-smoke）"
fi

_emit ""
if ((FAILED)); then
  _emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
  err "安装未完全成功"
  show_failure_context
  _emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
  exit 1
fi

_emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
_emit " ${C_GREEN}${C_BOLD}✓ Nerve WAF 已就绪${C_RESET}"
_emit ""
_emit " ${C_BOLD}访问地址${C_RESET}"
_emit "   Control API   ${C_CYAN}http://127.0.0.1:${CONTROLLER_PORT}${C_RESET}"
_emit "   健康检查      http://127.0.0.1:${CONTROLLER_PORT}/api/health"
_emit "   OpenAPI       http://127.0.0.1:${CONTROLLER_PORT}/api/openapi.json"
if ((HAS_UI)); then
  _emit "   Console       ${C_CYAN}http://127.0.0.1:${CONSOLE_PORT}/${C_RESET}"
else
  _emit "   Console       未启动（standalone；设置 NERVE_UI_DIR 可启用）"
fi
_emit "   Agent         http://127.0.0.1:${AGENT_PORT}/health"
_emit ""
_emit " ${C_BOLD}常用命令${C_RESET}"
_emit "   查看日志      docker compose ${FILES[*]} logs -f --tail=100"
_emit "   停止          bash install.sh --down"
_emit "   卸载          bash install.sh --uninstall"
_emit "   重新安装      bash install.sh --force-env"
_emit ""
_emit " ${C_YELLOW}安全提示${C_RESET}：当前 NERVE_AUTH_ENABLED=$(envval NERVE_AUTH_ENABLED 0)，"
_emit "   数据库口令与 API Key 均为开发占位值。生产上线前必读"
_emit "   docs/runbooks/PRODUCTION.md"
_emit "${C_BOLD}────────────────────────────────────────────────${C_RESET}"
