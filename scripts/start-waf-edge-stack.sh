#!/usr/bin/env bash
# 形态 B 一键启动：nerve-waf-api（控制面）+ nerve-waf-edge（数据面）+ 可选 Console
#
# 用法（WSL / Linux）:
#   cd Nerve-waf
#   bash scripts/start-waf-edge-stack.sh              # 后台启动 API + Edge + Console
#   bash scripts/start-waf-edge-stack.sh --no-console # 仅 API + Edge
#   bash scripts/start-waf-edge-stack.sh --build       # 强制重编
#   bash scripts/start-waf-edge-stack.sh --skip-build  # 绝不编译（缺二进制则失败）
#   bash scripts/start-waf-edge-stack.sh stop
#   bash scripts/start-waf-edge-stack.sh status
#
# 环境变量:
#   WAF_EDGE_STACK_API_PORT=8781    控制面端口
#   WAF_EDGE_STACK_EDGE_PORT=8080   数据面端口（接客户流量）
#   WAF_EDGE_STACK_CONSOLE_PORT=5177
#   WAF_EDGE_STACK_SKIP_BUILD=1
#   WAF_EDGE_STACK_FORCE_BUILD=1
#   WAF_EDGE_SYNC_MODE=watch       Edge 长轮询（默认）；poll 则定时 List
#   WAF_EDGE_POLL_SEC=5            poll 模式间隔

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CONFIG="${WAF_CONFIG:-$ROOT/config.yml}"
API_PORT="${WAF_EDGE_STACK_API_PORT:-8781}"
EDGE_PORT="${WAF_EDGE_STACK_EDGE_PORT:-8080}"
CONSOLE_PORT="${WAF_EDGE_STACK_CONSOLE_PORT:-5177}"
SKIP_BUILD="${WAF_EDGE_STACK_SKIP_BUILD:-0}"
FORCE_BUILD="${WAF_EDGE_STACK_FORCE_BUILD:-0}"
WITH_CONSOLE=1

STATE_DIR="$ROOT/data/waf-edge-stack"
API_PID="$STATE_DIR/api.pid"
EDGE_PID="$STATE_DIR/edge.pid"
CONSOLE_PID="$STATE_DIR/console.pid"
API_LOG="$ROOT/logs/waf-edge-stack-api.log"
EDGE_LOG="$ROOT/logs/waf-edge-stack-edge.log"
CONSOLE_LOG="$ROOT/logs/waf-edge-stack-console.log"
CONSOLE_DIR="$ROOT/apps/console-react"

API_URL="http://127.0.0.1:${API_PORT}"
EDGE_URL="http://127.0.0.1:${EDGE_PORT}"
CONSOLE_URL="http://127.0.0.1:${CONSOLE_PORT}"

green() { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
red() { printf '\033[31m%s\033[0m\n' "$*"; }
bold() { printf '\033[1m%s\033[0m\n' "$*"; }

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

load_dotenv() {
  if [[ -f "$ROOT/.env" ]]; then
    set -a
    # shellcheck disable=SC1090
    source <(sed 's/\r$//' "$ROOT/.env")
    set +a
    yellow "[env] loaded $ROOT/.env"
  fi
}

ensure_dirs() {
  mkdir -p "$STATE_DIR" "$ROOT/logs"
  export NERVE_WAF_DATA="${NERVE_WAF_DATA:-$STATE_DIR/waf-data}"
  mkdir -p "$NERVE_WAF_DATA"
}

# Edge 内联检测在 API/Edge 进程内跑 InspectPipeline；专家段需 Native Pack。
ensure_expert_env() {
  if [[ "${NERVE_EXPERT_ENGINE:-on}" == "off" || "${NERVE_EXPERT_ENGINE:-on}" == "0" ]]; then
    yellow "[expert] NERVE_EXPERT_ENGINE=off，专家段不加载规则包"
    return 0
  fi
  if [[ -n "${NERVE_EXPERT_PACK:-}" ]]; then
    yellow "[expert] NERVE_EXPERT_PACK=$NERVE_EXPERT_PACK"
    export NERVE_EXPERT_ENGINE="${NERVE_EXPERT_ENGINE:-on}"
    export NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-1}"
    return 0
  fi
  local mono native_root pack_name pack_path
  mono="$(cd "$ROOT/../.." && pwd)"
  native_root="${NATIVE_ROOT:-$ROOT/../Nerve-antibody}"
  pack_name="${NERVE_WAF_EXPERT_PACK_NAME:-baseline}"
  pack_path="$native_root/packs/$pack_name"
  if [[ ! -f "$pack_path/pack.toml" ]]; then
    yellow "[expert] 未找到 $pack_path/pack.toml"
    yellow "[expert] 流水线仅开 expert 且关 core 时 SQLi 不会被拦；请设置 NERVE_EXPERT_PACK 或编译 Nerve-antibody"
    return 0
  fi
  export NERVE_EXPERT_ENGINE=on
  export NERVE_EXPERT_PACK="$pack_path"
  export NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-1}"
  yellow "[expert] NERVE_EXPERT_PACK=$NERVE_EXPERT_PACK"
}

stop_pid() {
  local file="$1" label="$2"
  if [[ -f "$file" ]]; then
    local pid
    pid="$(cat "$file" 2>/dev/null || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.5
      kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
      yellow "[stop] $label (pid $pid)"
    fi
    rm -f "$file"
  fi
}

free_port() {
  local port="$1"
  if command -v fuser >/dev/null 2>&1; then
    fuser -k "${port}/tcp" 2>/dev/null || true
  elif command -v lsof >/dev/null 2>&1; then
    lsof -ti:"$port" 2>/dev/null | xargs -r kill 2>/dev/null || true
  fi
}

build_bins() {
  # shellcheck source=lib/ensure-rust-bins.sh
  source "$(cd "$(dirname "$0")" && pwd)/lib/ensure-rust-bins.sh"
  local -a args=()
  [[ "$FORCE_BUILD" == "1" ]] && args+=(--force)
  [[ "$SKIP_BUILD" == "1" ]] && args+=(--skip)
  ENSURE_RUST_FORCE="$FORCE_BUILD" ENSURE_RUST_SKIP="$SKIP_BUILD" \
    ensure_rust_release_bins "$ROOT" "${args[@]}" nerve-waf-api nerve-waf-edge
}

wait_http() {
  local url="$1" label="$2" pid_file="$3" log="$4" tries="${5:-45}"
  yellow "[wait] $label → $url"
  for _ in $(seq 1 "$tries"); do
    if curl -sf --max-time 2 "$url" >/dev/null 2>&1; then
      green "[wait] $label 就绪"
      return 0
    fi
    if [[ -f "$pid_file" ]]; then
      local pid
      pid="$(cat "$pid_file")"
      if ! kill -0 "$pid" 2>/dev/null; then
        red "[wait] $label 已退出，日志:"
        tail -n 40 "$log" >&2 || true
        exit 1
      fi
    fi
    sleep 1
  done
  red "[wait] $label 超时"
  tail -n 50 "$log" >&2 || true
  exit 1
}

start_api() {
  local bin="$ROOT/target/release/nerve-waf-api"
  [[ -x "$bin" ]] || { red "缺少 $bin，去掉 --skip-build 或先 cargo build"; exit 1; }
  stop_pid "$API_PID" "nerve-waf-api"
  free_port "$API_PORT"
  yellow "[api] $bin :$API_PORT"
  if [[ -n "${DATABASE_URL:-}" ]]; then
    yellow "[api] DATABASE_URL → PostgreSQL"
  else
    yellow "[api] file store（或 config.yml database.url）"
  fi
  local -a args=(--config "$CONFIG" --port "$API_PORT")
  [[ -n "${DATABASE_URL:-}" ]] && args+=(--database-url "$DATABASE_URL")
  nohup env PORT="$API_PORT" DATABASE_URL="${DATABASE_URL:-}" NERVE_WAF_DATA="$NERVE_WAF_DATA" \
    NERVE_EXPERT_ENGINE="${NERVE_EXPERT_ENGINE:-}" \
    NERVE_EXPERT_PACK="${NERVE_EXPERT_PACK:-}" \
    NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-}" \
    "$bin" "${args[@]}" >> "$API_LOG" 2>&1 &
  echo $! > "$API_PID"
}

start_edge() {
  local bin="$ROOT/target/release/nerve-waf-edge"
  [[ -x "$bin" ]] || { red "缺少 $bin"; exit 1; }
  stop_pid "$EDGE_PID" "nerve-waf-edge"
  free_port "$EDGE_PORT"
  local plugins_dir="${NERVE_WAF_PLUGINS_DIR:-${NERVE_WAF_PLUGINS:-$ROOT/plugins}}"
  yellow "[edge] $bin :$EDGE_PORT ← dataplane $API_URL"
  yellow "[edge] plugins_dir=$plugins_dir"
  nohup env WAF_EDGE_PORT="$EDGE_PORT" WAF_EDGE_API="$API_URL" \
    WAF_EDGE_SYNC_MODE="${WAF_EDGE_SYNC_MODE:-watch}" \
    WAF_EDGE_POLL_SEC="${WAF_EDGE_POLL_SEC:-5}" \
    NERVE_WAF_DATA="$NERVE_WAF_DATA" \
    NERVE_WAF_PLUGINS_DIR="$plugins_dir" \
    NERVE_EXPERT_ENGINE="${NERVE_EXPERT_ENGINE:-}" \
    NERVE_EXPERT_PACK="${NERVE_EXPERT_PACK:-}" \
    NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-}" \
    "$bin" --port "$EDGE_PORT" --api "$API_URL" --plugins-dir "$plugins_dir" \
    >> "$EDGE_LOG" 2>&1 &
  echo $! > "$EDGE_PID"
}

start_console() {
  if [[ "$WITH_CONSOLE" != "1" ]]; then
    yellow "[console] 跳过 (--no-console)"
    return
  fi
  command -v node >/dev/null 2>&1 || { yellow "[console] 无 node，跳过"; return; }
  command -v npm >/dev/null 2>&1 || { yellow "[console] 无 npm，跳过"; return; }
  [[ -d "$CONSOLE_DIR" ]] || { yellow "[console] 无 $CONSOLE_DIR，跳过"; return; }

  local lib_dir script_dir
  lib_dir="$(cd "$(dirname "$0")/lib" && pwd)"
  script_dir="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck source=npm-deps.sh
  source "$script_dir/npm-deps.sh"
  ensure_npm_deps "$CONSOLE_DIR"

  stop_pid "$CONSOLE_PID" "console-react"
  free_port "$CONSOLE_PORT"

  # WSL drvfs（/mnt/*）inotify 上限低，Vite 易 EMFILE；与 start-nerve-waf-expert 一致
  local -a console_env=(NERVE_WAF_API_PORT="$API_PORT" VITE_CONSOLE_TIME_ZONE="${VITE_CONSOLE_TIME_ZONE:-Asia/Shanghai}")
  if [[ "$ROOT" == /mnt/* ]] || [[ -n "${WSL_DISTRO_NAME:-}" ]]; then
    ulimit -n 65536 2>/dev/null || ulimit -n 8192 2>/dev/null || true
    if [[ "${VITE_USE_POLLING:-}" != "0" ]]; then
      console_env+=(VITE_USE_POLLING=1 CHOKIDAR_USEPOLLING=1)
      yellow "[console] WSL drvfs：VITE_USE_POLLING=1（避免 EMFILE；禁用时 export VITE_USE_POLLING=0）"
    fi
  fi

  yellow "[console] $CONSOLE_URL (/api → :$API_PORT)"
  (
    cd "$CONSOLE_DIR"
    nohup env "${console_env[@]}" npm run dev >> "$CONSOLE_LOG" 2>&1 &
    echo $! > "$CONSOLE_PID"
  )
}

cmd_start() {
  load_dotenv
  ensure_dirs
  ensure_expert_env
  build_bins
  start_api
  wait_http "$API_URL/api/health" "API" "$API_PID" "$API_LOG"
  start_edge
  wait_http "$EDGE_URL/health" "WAF Edge" "$EDGE_PID" "$EDGE_LOG" 30
  start_console
  if [[ "$WITH_CONSOLE" == "1" ]] && [[ -f "$CONSOLE_PID" ]]; then
    wait_http "$CONSOLE_URL/" "Console" "$CONSOLE_PID" "$CONSOLE_LOG" 60 || true
  fi

  bold ""
  bold "形态 B 已启动（WAF 边缘回源，无需 Gateway）"
  echo "  控制面 API:  $API_URL"
  echo "  数据面 Edge: $EDGE_URL  ← 客户流量打这里（Host 路由 + 检测 + 回源）"
  if [[ "$WITH_CONSOLE" == "1" ]] && [[ -f "$CONSOLE_PID" ]]; then
    echo "  Console:     $CONSOLE_URL"
  fi
  echo ""
  echo "  验收 dataplane: curl -s $API_URL/api/waf/edge/dataplane | head"
  echo "  Edge 同步状态:  curl -s $API_URL/api/waf/edge/status"
  echo "  打流量示例:     curl -H \"Host: <应用域名>\" $EDGE_URL/"
  echo "  停止:           bash scripts/start-waf-edge-stack.sh stop"
  echo "  日志:           API  $API_LOG"
  echo "                  Edge $EDGE_LOG"
  bold ""
}

cmd_stop() {
  stop_pid "$API_PID" "nerve-waf-api"
  stop_pid "$EDGE_PID" "nerve-waf-edge"
  stop_pid "$CONSOLE_PID" "console-react"
  free_port "$API_PORT"
  free_port "$EDGE_PORT"
  free_port "$CONSOLE_PORT"
  green "已停止 WAF Edge 栈"
}

cmd_status() {
  bold "WAF Edge 栈状态（形态 B）"
  echo "  API:     $API_URL"
  echo "  Edge:    $EDGE_URL"
  echo "  Console: $CONSOLE_URL"
  for pair in "$API_PID:API" "$EDGE_PID:Edge" "$CONSOLE_PID:Console"; do
    local file="${pair%%:*}" label="${pair##*:}"
    if [[ -f "$file" ]]; then
      local pid
      pid="$(cat "$file")"
      if kill -0 "$pid" 2>/dev/null; then
        green "  $label: running (pid $pid)"
      else
        red "  $label: stale pid $pid"
      fi
    else
      yellow "  $label: 未启动"
    fi
  done
}

CMD="start"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --build) FORCE_BUILD=1; shift ;;
    --no-console) WITH_CONSOLE=0; shift ;;
    start|stop|status) CMD="$1"; shift ;;
    *) red "未知参数: $1"; usage 1 ;;
  esac
done

case "$CMD" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  *) usage 1 ;;
esac
