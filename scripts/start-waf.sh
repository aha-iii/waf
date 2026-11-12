#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# WSL/Linux 本地联调：启动 controller + agent + console 三进程。
#
#   cd Nerve-waf/Nerve-waf
#   bash scripts/start-waf.sh
#   bash scripts/start-waf.sh --build       # 先 cargo + console 构建
#   bash scripts/start-waf.sh --stop        # 停止上次后台进程
#   bash scripts/start-waf.sh --no-console  # 仅 controller + agent
#
# Windows 访问 WSL 端口（LAN / 本机非 localhost 场景）：
#   管理员 PowerShell：powershell -File scripts/wsl-port-forward.ps1
#   或：powershell -File scripts/start-waf.ps1 -PortForward -Build
#
# 默认端口：controller 9000 · agent 9080 · console 9090（见 docs/PORTS.md）

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

RUN_DIR="${NERVE_WAF_RUN_DIR:-$ROOT/.run}"
PID_FILE="$RUN_DIR/start-waf.pid"
LOG_DIR="$RUN_DIR/logs"

DO_BUILD=false
DO_STOP=false
SKIP_CONSOLE=false

usage() {
  cat <<EOF
usage: $(basename "$0") [--build] [--stop] [--no-console]

  --build       强制 cargo build + console dist 构建后再启动
  --stop        停止由本脚本启动的后台进程
  --no-console  不启动 nerve-waf-console（仅 controller + agent）

环境：可 source .env；日志目录 ${LOG_DIR}/
EOF
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1" >&2
    exit 1
  }
}

load_env() {
  if [[ -f "$ROOT/.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    source <(sed 's/\r$//' "$ROOT/.env")
    set +a
    echo "[start-waf] loaded .env"
  fi
}

stop_processes() {
  if [[ ! -f "$PID_FILE" ]]; then
    echo "[start-waf] no pid file ($PID_FILE)"
    return 0
  fi
  echo "[start-waf] stopping …"
  local svc pid _
  while read -r svc pid _; do
    [[ -n "${pid:-}" ]] || continue
    if kill -0 "$pid" 2>/dev/null; then
      echo "  stop ${svc:-proc} (pid $pid)"
      kill -TERM "$pid" 2>/dev/null || true
    fi
  done < "$PID_FILE"
  sleep 1
  while read -r _ pid _; do
    [[ -n "${pid:-}" ]] || continue
    kill -KILL "$pid" 2>/dev/null || true
  done < "$PID_FILE"
  rm -f "$PID_FILE"
}

port_in_use() {
  local port="$1"
  if command -v ss >/dev/null 2>&1; then
    ss -tln 2>/dev/null | awk '{print $4}' | grep -qE ":${port}$"
  elif command -v netstat >/dev/null 2>&1; then
    netstat -tln 2>/dev/null | grep -qE ":${port}[[:space:]]"
  else
    return 1
  fi
}

resolve_controller_port() {
  local p="${PORT:-9000}"
  if port_in_use "$p"; then
    echo "[start-waf] WARN: :${p} 已被占用（常见：Docker nerve-clickhouse 占 9000）" >&2
    if [[ "$p" == "9000" ]] && ! port_in_use 9002; then
      p=9002
      echo "[start-waf] WARN: controller 改用 :9002（Windows portproxy 的 9000 仍指向 ClickHouse，Console :9090 可正常用）" >&2
    else
      echo "[start-waf] ERROR: 无可用 controller 端口，请改 .env 中 PORT 或停止占用进程" >&2
      exit 1
    fi
  fi
  PORT="$p"
  export PORT
  export WAF_CONTROLLER_UPSTREAM="http://127.0.0.1:${PORT}"
  export WAF_AGENT_CONTROLLER="http://127.0.0.1:${PORT}"
}

wait_http() {
  local url="$1" label="$2" tries="${3:-45}"
  need_cmd curl
  for ((i = 1; i <= tries; i++)); do
    if curl -sf "$url" >/dev/null 2>&1; then
      echo "[start-waf] ${label} ready (${url})"
      return 0
    fi
    sleep 1
  done
  echo "[start-waf] ERROR: ${label} not ready (${url})" >&2
  echo "  log: ${LOG_DIR}/${label}.log" >&2
  return 1
}

ensure_bins() {
  local bin_controller="$1" bin_agent="$2" bin_console="$3"
  if [[ "$DO_BUILD" == "true" ]] \
    || [[ ! -x "$bin_controller" ]] \
    || [[ ! -x "$bin_agent" ]] \
    || [[ ! -x "$bin_console" ]]; then
    echo "[start-waf] cargo build --release -p nerve-waf-controller -p nerve-waf-agent"
    need_cmd cargo
    cargo build --release -p nerve-waf-controller -p nerve-waf-agent
  fi
}

build_console_if_needed() {
  local console_dist="$1"
  if [[ "$SKIP_CONSOLE" == "true" ]]; then
    return 0
  fi
  if [[ -f "${console_dist}/index.html" ]]; then
    return 0
  fi
  if [[ "$DO_BUILD" != "true" ]]; then
    return 0
  fi
  echo "[start-waf] building console → ${console_dist}"
  if [[ -f "$ROOT/ops/scripts/lib/console-build.sh" ]]; then
    # shellcheck source=/dev/null
    source "$ROOT/ops/scripts/lib/console-build.sh"
    console_build_init "$ROOT"
    CONSOLE_DIR="$ROOT/apps/console"
    CONSOLE_DIST="$console_dist"
    ensure_console_dist || return 1
    return 0
  fi
  need_cmd npm
  (cd "$ROOT/apps/console" && npm install && npm run build)
}

record_pid() {
  printf '%s %s\n' "$1" "$2" >>"$PID_FILE"
}

start_bg() {
  local svc_name="$1"
  local log="$LOG_DIR/${svc_name}.log"
  shift
  echo "[start-waf] start ${svc_name} → ${log}"
  "$@" >>"$log" 2>&1 &
  record_pid "$svc_name" "$!"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) DO_BUILD=true ;;
    --stop) DO_STOP=true ;;
    --no-console) SKIP_CONSOLE=true ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

if [[ "$DO_STOP" == "true" ]]; then
  stop_processes
  exit 0
fi

load_env

export PORT="${PORT:-9000}"
export WAF_AGENT_PORT="${WAF_AGENT_PORT:-9080}"
export CONSOLE_PORT="${CONSOLE_PORT:-9090}"
export WAF_CONSOLE_PORT="${WAF_CONSOLE_PORT:-$CONSOLE_PORT}"
export NERVE_AUTH_ENABLED="${NERVE_AUTH_ENABLED:-0}"
export NERVE_CORS_ORIGINS="${NERVE_CORS_ORIGINS:-*}"
export NERVE_WAF_PLUGINS_DIR="${NERVE_WAF_PLUGINS_DIR:-$ROOT/plugins}"
export WAF_CONFIG="${WAF_CONFIG:-$ROOT/config.yml}"

resolve_controller_port

CONSOLE_DIST="$ROOT/apps/console/dist"
export WAF_CONSOLE_DIR="${WAF_CONSOLE_DIR:-$CONSOLE_DIST}"

BIN_DIR="$ROOT/target/release"
BIN_CONTROLLER="$BIN_DIR/nerve-waf-controller"
BIN_AGENT="$BIN_DIR/nerve-waf-agent"
BIN_CONSOLE="$BIN_DIR/nerve-waf-console"

if [[ -f "$PID_FILE" ]]; then
  echo "[start-waf] found existing pid file — stopping previous run"
  stop_processes
fi

mkdir -p "$RUN_DIR" "$LOG_DIR" "$ROOT/data" "$ROOT/logs"

ensure_bins "$BIN_CONTROLLER" "$BIN_AGENT" "$BIN_CONSOLE"
build_console_if_needed "$CONSOLE_DIST"

: >"$PID_FILE"

cleanup() {
  echo
  echo "[start-waf] shutting down …"
  stop_processes
  exit 0
}
trap cleanup INT TERM

echo "============================================"
echo " Nerve WAF — local (controller/agent/console)"
echo " Controller : http://127.0.0.1:${PORT}/api/health"
echo " Agent      : http://127.0.0.1:${WAF_AGENT_PORT}/health"
if [[ "$SKIP_CONSOLE" != "true" && -f "${WAF_CONSOLE_DIR}/index.html" ]]; then
  echo " Console    : http://127.0.0.1:${CONSOLE_PORT}/"
fi
echo " Logs       : ${LOG_DIR}/"
if grep -qi microsoft /proc/version 2>/dev/null; then
  echo " Windows    : 管理员 PS → scripts/wsl-port-forward.ps1 (9000/9080/9090)"
fi
echo "============================================"

start_bg controller "$BIN_CONTROLLER"
wait_http "http://127.0.0.1:${PORT}/api/health" controller || {
  stop_processes
  exit 1
}

start_bg agent "$BIN_AGENT" --port "$WAF_AGENT_PORT" --control "$WAF_AGENT_CONTROLLER"
wait_http "http://127.0.0.1:${WAF_AGENT_PORT}/health" agent || {
  stop_processes
  exit 1
}

if [[ "$SKIP_CONSOLE" != "true" ]]; then
  if [[ ! -f "${WAF_CONSOLE_DIR}/index.html" ]]; then
    echo "[start-waf] WARN: no console dist at ${WAF_CONSOLE_DIR}"
    echo "         run: bash scripts/start-waf.sh --build"
    echo "         or:  cd apps/console && npm run dev"
  else
    start_bg console "$BIN_CONSOLE"
    wait_http "http://127.0.0.1:${CONSOLE_PORT}/" console || {
      stop_processes
      exit 1
    }
  fi
fi

echo "[start-waf] running (Ctrl+C to stop all)"
wait
