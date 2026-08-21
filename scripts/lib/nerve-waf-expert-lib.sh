# Nerve-waf + Native Expert 共享变量与函数（由 start/test 脚本 source）

nerve_antibody_lib_init() {
  ROOT="$(cd "$(dirname "${BASH_SOURCE[1]}")/.." && pwd)"
  MONO="$(cd "$ROOT/../.." && pwd)"
  NATIVE_ROOT="${NATIVE_ROOT:-$ROOT/../Nerve-antibody}"
  CONFIG="${WAF_CONFIG:-$ROOT/config.yml}"
  CONSOLE_DIR="$ROOT/apps/console-react"

  MODE="${NERVE_WAF_EXPERT_MODE:-dev}"
  PORT="${NERVE_WAF_EXPERT_PORT:-8782}"
  EDGE_PORT="${NERVE_WAF_EXPERT_EDGE_PORT:-8080}"
  CONSOLE_PORT="${NERVE_WAF_EXPERT_CONSOLE_PORT:-5177}"
  PACK="${NERVE_WAF_EXPERT_PACK_NAME:-baseline}"
  SKIP_BUILD="${NERVE_WAF_EXPERT_SKIP_BUILD:-0}"
  SKIP_CONSOLE="${NERVE_WAF_EXPERT_SKIP_CONSOLE:-${NERVE_WAF_EXPERT_SKIP_DASHBOARD:-1}}"
  SKIP_EDGE="${NERVE_WAF_EXPERT_SKIP_EDGE:-0}"
  API="http://127.0.0.1:${PORT}"
  EDGE_URL="http://127.0.0.1:${EDGE_PORT}"
  CONSOLE_URL="http://127.0.0.1:${CONSOLE_PORT}"

  STATE_DIR="$ROOT/data/nerve-waf-expert"
  RUNTIME_ENV="$STATE_DIR/runtime.env"
  WAF_PID_FILE="$STATE_DIR/waf.pid"
  EDGE_PID_FILE="$STATE_DIR/edge.pid"
  LICENSE_PID_FILE="$STATE_DIR/license.pid"
  CONSOLE_PID_FILE="$STATE_DIR/console.pid"
  WAF_LOG="$ROOT/logs/nerve-waf-expert.log"
  EDGE_LOG="$ROOT/logs/nerve-waf-edge.log"
  LICENSE_LOG="$ROOT/logs/nerve-waf-expert-license.log"
  CONSOLE_LOG="$ROOT/logs/nerve-waf-expert-console.log"
  SEAL_KEY_FILE="${SEAL_KEY_FILE:-$NATIVE_ROOT/.secrets/pack-seal-key.b64}"
  SEALED_PACK="$NATIVE_ROOT/dist/customer/antibody/${PACK}.sealed"

  load_waf_dotenv
}

# 读取 Nerve-waf/.env（与 start-api.sh 一致）；Expert 不再强制 file 存储。
load_waf_dotenv() {
  local env_file="$ROOT/.env"
  if [[ ! -f "$env_file" ]]; then
    return 0
  fi
  set -a
  # shellcheck disable=SC1090
  source <(sed 's/\r$//' "$env_file")
  set +a
}

# NERVE_WAF_EXPERT_FILE_STORE=1 时仍用 file 存储（旧 Expert 默认行为）。
apply_expert_database_env() {
  if [[ "${NERVE_WAF_EXPERT_FILE_STORE:-0}" == "1" ]]; then
    unset DATABASE_URL
    return 0
  fi
  if [[ -n "${DATABASE_URL:-}" ]]; then
    export DATABASE_URL
  else
    unset DATABASE_URL
  fi
}

green() { printf '\033[32m%s\033[0m\n' "$*"; }
red()   { printf '\033[31m%s\033[0m\n' "$*"; }
yellow(){ printf '\033[33m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || { red "缺少命令: $1"; exit 1; }
}

ensure_state_dir() {
  mkdir -p "$STATE_DIR" "$ROOT/logs"
  if [[ -n "${NERVE_WAF_DATA:-}" ]]; then
    mkdir -p "$NERVE_WAF_DATA"
  fi
}

stop_pid_file() {
  local file="$1" label="$2"
  if [[ -f "$file" ]]; then
    local pid
    pid="$(cat "$file" 2>/dev/null || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.5
      kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
      yellow "[stop] 已停止 $label (pid $pid)"
    fi
    rm -f "$file"
  fi
}

free_listen_port() {
  local port="$1"
  if command -v fuser >/dev/null 2>&1; then
    fuser -k "${port}/tcp" 2>/dev/null || true
  elif command -v lsof >/dev/null 2>&1; then
    lsof -ti:"$port" 2>/dev/null | xargs -r kill 2>/dev/null || true
  fi
}

resolve_pack_path() {
  if [[ "$MODE" == "sealed" ]]; then
    echo "$SEALED_PACK"
    return
  fi
  local plain="$NATIVE_ROOT/packs/$PACK"
  if [[ -f "$plain/pack.toml" ]]; then
    echo "$plain"
    return
  fi
  red "找不到 pack: $plain/pack.toml"
  exit 1
}

ensure_sealed_pack() {
  need_cmd openssl
  mkdir -p "$(dirname "$SEAL_KEY_FILE")" "$(dirname "$SEALED_PACK")"
  if [[ ! -f "$SEAL_KEY_FILE" ]]; then
    yellow "[seal] 生成密钥 $SEAL_KEY_FILE"
    openssl rand -base64 32 > "$SEAL_KEY_FILE"
  fi
  yellow "[seal] 加密 packs/$PACK -> $SEALED_PACK"
  export NERVE_PACK_SEAL_KEY_B64="$(tr -d '\n\r' < "$SEAL_KEY_FILE")"
  (cd "$NATIVE_ROOT" && cargo run -q -p nerve-antibody-pack --bin seal-pack -- \
    "packs/$PACK" "$SEALED_PACK")
}

ensure_console_built() {
  local lib_dir
  lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  # shellcheck source=console-build.sh
  source "$lib_dir/console-build.sh"
  console_build_init "$ROOT"
  ensure_console_dist
}

export_expert_env() {
  local pack_path
  pack_path="$(resolve_pack_path)"

  export NERVE_EXPERT_ENGINE=on
  export NERVE_EXPERT_PACK="$pack_path"
  export NERVE_AUTH_ENABLED="${NERVE_AUTH_ENABLED:-1}"
  export NERVE_AUTH_MODE="${NERVE_AUTH_MODE:-local}"
  export KEYCLOAK_ENABLED="${KEYCLOAK_ENABLED:-0}"
  export NERVE_API_KEY="${NERVE_API_KEY:-dev-nerve-key}"
  export PORT="$PORT"
  export NERVE_WAF_DATA="${NERVE_WAF_DATA:-$STATE_DIR/waf-data}"
  local console_dist="$ROOT/apps/console-react/dist"
  if [[ -f "$console_dist/index.html" ]]; then
    export WAF_CONSOLE_DIR="$console_dist"
  fi
  apply_expert_database_env

  if [[ "$MODE" == "dev" ]]; then
    export NERVE_NATIVE_DEV_PACK=1
    unset NERVE_LICENSE_KEY NERVE_LICENSE_URL NERVE_LICENSE_DEV NERVE_LICENSE_PACK_KEY_B64
  else
    unset NERVE_NATIVE_DEV_PACK
    export NERVE_LICENSE_KEY="${NERVE_LICENSE_KEY:-demo-license-key}"
    export NERVE_LICENSE_URL="${NERVE_LICENSE_URL:-http://127.0.0.1:8799/v1/activate}"
  fi
}

write_runtime_env() {
  ensure_state_dir
  cat > "$RUNTIME_ENV" <<EOF
API=$API
PORT=$PORT
EDGE_URL=$EDGE_URL
EDGE_PORT=$EDGE_PORT
CONSOLE_URL=$CONSOLE_URL
MODE=$MODE
PACK=$PACK
NERVE_EXPERT_PACK=$NERVE_EXPERT_PACK
WAF_LOG=$WAF_LOG
CONSOLE_LOG=$CONSOLE_LOG
EOF
}

load_runtime_env() {
  if [[ -f "$RUNTIME_ENV" ]]; then
    # shellcheck disable=SC1090
    source "$RUNTIME_ENV"
    API="${API:-http://127.0.0.1:${PORT:-8782}}"
    CONSOLE_URL="${CONSOLE_URL:-${DASHBOARD_URL:-http://127.0.0.1:${CONSOLE_PORT:-5177}}}"
  fi
}

build_waf_expert() {
  if [[ "$SKIP_BUILD" == "1" ]]; then
    yellow "[build] 跳过 (NERVE_WAF_EXPERT_SKIP_BUILD=1)"
    return
  fi
  local lib_dir
  lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  # shellcheck source=ensure-rust-bins.sh
  source "$lib_dir/ensure-rust-bins.sh"

  local force=0
  [[ "${NERVE_WAF_EXPERT_FORCE_BUILD:-0}" == "1" ]] && force=1

  local api_bin="$ROOT/target/release/nerve-waf-api"
  local edge_bin="$ROOT/target/release/nerve-waf-edge"
  local need_native=0
  if [[ ! -f "$NATIVE_ROOT/target/release/libnerve_antibody.rlib" ]] || [[ "$force" == "1" ]]; then
    need_native=1
  fi

  if [[ "$need_native" == "1" ]]; then
    yellow "[build] 编译 Antibody 引擎 (release)..."
    (cd "$NATIVE_ROOT" && cargo build -q --release -p nerve-antibody-dsl -p nerve-antibody-pack -p nerve-antibody-engine -p nerve-antibody)
  else
    yellow "[build] 跳过 native（已存在；NERVE_WAF_EXPERT_FORCE_BUILD=1 可强制）"
  fi

  local -a rust_args=()
  [[ "$force" == "1" ]] && rust_args+=(--force)
  ensure_rust_release_bins "$ROOT" "${rust_args[@]}" nerve-waf-api nerve-waf-edge
  green "[build] OK: $api_bin (+ nerve-waf-edge)"
}

start_mock_license() {
  need_cmd python3
  ensure_state_dir
  stop_pid_file "$LICENSE_PID_FILE" "mock-license"
  free_listen_port 8799
  export MOCK_PACK_KEY_B64="$(tr -d '\n\r' < "$SEAL_KEY_FILE")"
  export MOCK_LISTEN="${MOCK_LISTEN:-127.0.0.1:8799}"
  yellow "[license] Mock License $MOCK_LISTEN"
  nohup python3 "$NATIVE_ROOT/scripts/license-server-mock.py" \
    >> "$LICENSE_LOG" 2>&1 &
  echo $! > "$LICENSE_PID_FILE"
  sleep 0.8
}

start_waf_expert() {
  ensure_state_dir
  stop_pid_file "$WAF_PID_FILE" "nerve-waf-api"
  free_listen_port "$PORT"
  local bin="$ROOT/target/release/nerve-waf-api"
  [[ -x "$bin" ]] || { red "缺少 $bin，请先编译或去掉 --skip-build"; exit 1; }

  yellow "[waf] 启动 $bin (port $PORT)"
  if [[ -n "${DATABASE_URL:-}" ]]; then
    yellow "[waf] DATABASE_URL -> PostgreSQL ($(echo "$DATABASE_URL" | sed 's/:.*@/:***@/'))"
  else
    yellow "[waf] DATABASE_URL unset -> file store (或 config.yml database.url)"
  fi

  local -a waf_args=(--config "$CONFIG" --port "$PORT")
  if [[ -n "${DATABASE_URL:-}" ]]; then
    waf_args+=(--database-url "$DATABASE_URL")
  fi

  nohup env \
    NERVE_WAF_DATA="$NERVE_WAF_DATA" \
    NERVE_EXPERT_ENGINE="$NERVE_EXPERT_ENGINE" \
    NERVE_EXPERT_PACK="$NERVE_EXPERT_PACK" \
    NERVE_AUTH_ENABLED="$NERVE_AUTH_ENABLED" \
    NERVE_AUTH_MODE="${NERVE_AUTH_MODE:-local}" \
    NERVE_AUTH_JWT_SECRET="${NERVE_AUTH_JWT_SECRET:-}" \
    NERVE_API_KEY="${NERVE_API_KEY:-dev-nerve-key}" \
    KEYCLOAK_ENABLED="${KEYCLOAK_ENABLED:-0}" \
    KEYCLOAK_AUTH_SERVER_URL="${KEYCLOAK_AUTH_SERVER_URL:-}" \
    KEYCLOAK_REALM="${KEYCLOAK_REALM:-}" \
    KEYCLOAK_CLIENT_ID="${KEYCLOAK_CLIENT_ID:-}" \
    NERVE_LICENSE_KEY="${NERVE_LICENSE_KEY:-}" \
    NERVE_LICENSE_URL="${NERVE_LICENSE_URL:-}" \
    NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-}" \
    WAF_CONSOLE_DIR="${WAF_CONSOLE_DIR:-}" \
    PORT="$PORT" \
    DATABASE_URL="${DATABASE_URL:-}" \
    "$bin" "${waf_args[@]}" \
    >> "$WAF_LOG" 2>&1 &
  echo $! > "$WAF_PID_FILE"
}

start_waf_edge() {
  if [[ "$SKIP_EDGE" == "1" ]]; then
    yellow "[edge] 跳过 (NERVE_WAF_EXPERT_SKIP_EDGE=1)"
    return
  fi

  ensure_state_dir
  stop_pid_file "$EDGE_PID_FILE" "nerve-waf-edge"
  free_listen_port "$EDGE_PORT"
  local bin="$ROOT/target/release/nerve-waf-edge"
  [[ -x "$bin" ]] || { red "缺少 $bin，请先编译或去掉 --skip-build"; exit 1; }

  local plugins_dir="${NERVE_WAF_PLUGINS_DIR:-${NERVE_WAF_PLUGINS:-$ROOT/plugins}}"
  yellow "[edge] 启动 $bin (port $EDGE_PORT, dataplane from $API)"
  yellow "[edge] plugins_dir=$plugins_dir"
  nohup env \
    WAF_EDGE_PORT="$EDGE_PORT" \
    WAF_EDGE_API="$API" \
    NERVE_WAF_DATA="$NERVE_WAF_DATA" \
    NERVE_WAF_PLUGINS_DIR="$plugins_dir" \
    NERVE_EXPERT_ENGINE="${NERVE_EXPERT_ENGINE:-}" \
    NERVE_EXPERT_PACK="${NERVE_EXPERT_PACK:-}" \
    NERVE_NATIVE_DEV_PACK="${NERVE_NATIVE_DEV_PACK:-}" \
    "$bin" --port "$EDGE_PORT" --api "$API" --plugins-dir "$plugins_dir" \
    >> "$EDGE_LOG" 2>&1 &
  echo $! > "$EDGE_PID_FILE"
}

wait_waf_edge() {
  if [[ "$SKIP_EDGE" == "1" ]]; then
    return
  fi

  yellow "[wait] $EDGE_URL/health"
  for _ in $(seq 1 30); do
    if curl -sf --max-time 2 "$EDGE_URL/health" >/dev/null 2>&1; then
      green "[wait] WAF Edge 就绪"
      return 0
    fi
    if [[ -f "$EDGE_PID_FILE" ]]; then
      local pid
      pid="$(cat "$EDGE_PID_FILE")"
      if ! kill -0 "$pid" 2>/dev/null; then
        red "[wait] WAF Edge 已退出，日志:"
        tail -n 40 "$EDGE_LOG" >&2 || true
        exit 1
      fi
    fi
    sleep 1
  done
  red "[wait] WAF Edge 超时"
  tail -n 50 "$EDGE_LOG" >&2 || true
  exit 1
}

ensure_console_deps() {
  need_cmd node npm
  [[ -d "$CONSOLE_DIR" ]] || { red "缺少 Console: $CONSOLE_DIR"; exit 1; }

  local node_major
  node_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
  if [[ "$node_major" -lt 18 ]]; then
    red "Node.js >= 18 required (current: $(node -v 2>/dev/null || echo missing))"
    exit 1
  fi

  local lib_dir script_dir
  lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  script_dir="$(cd "$lib_dir/.." && pwd)"
  # shellcheck source=../npm-deps.sh
  source "$script_dir/npm-deps.sh"
  ensure_npm_deps "$CONSOLE_DIR"
}

start_waf_console() {
  if [[ "$SKIP_CONSOLE" == "1" ]]; then
    yellow "[console] 跳过 (NERVE_WAF_EXPERT_SKIP_CONSOLE=1)"
    return
  fi

  ensure_state_dir
  ensure_console_deps
  stop_pid_file "$CONSOLE_PID_FILE" "console-react"
  free_listen_port "$CONSOLE_PORT"

  if [[ "$(pwd)" == /mnt/* ]] || [[ -n "${WSL_DISTRO_NAME:-}" ]]; then
    ulimit -n 65536 2>/dev/null || true
  fi

  yellow "[console] 启动 console-react ($CONSOLE_URL, /api → :$PORT)"
  (
    cd "$CONSOLE_DIR"
    nohup env NERVE_WAF_API_PORT="$PORT" npm run dev >> "$CONSOLE_LOG" 2>&1 &
    echo $! > "$CONSOLE_PID_FILE"
  )
}

wait_waf_console() {
  if [[ "$SKIP_CONSOLE" == "1" ]]; then
    return
  fi

  yellow "[wait] $CONSOLE_URL"
  for _ in $(seq 1 60); do
    if curl -sf --max-time 2 "$CONSOLE_URL/" >/dev/null 2>&1; then
      green "[wait] Console 就绪"
      return 0
    fi
    if [[ -f "$CONSOLE_PID_FILE" ]]; then
      local pid
      pid="$(cat "$CONSOLE_PID_FILE")"
      if ! kill -0 "$pid" 2>/dev/null; then
        red "[wait] Console 已退出，日志:"
        tail -n 40 "$CONSOLE_LOG" >&2 || true
        exit 1
      fi
    fi
    sleep 1
  done
  red "[wait] Console 超时"
  tail -n 50 "$CONSOLE_LOG" >&2 || true
  exit 1
}

wait_waf_api() {
  yellow "[wait] $API/api/health"
  for _ in $(seq 1 45); do
    if curl -sf --max-time 2 "$API/api/health" >/dev/null 2>&1; then
      green "[wait] API 就绪"
      return 0
    fi
    if [[ -f "$WAF_PID_FILE" ]]; then
      local pid
      pid="$(cat "$WAF_PID_FILE")"
      if ! kill -0 "$pid" 2>/dev/null; then
        red "[wait] WAF 已退出，日志:"
        tail -n 40 "$WAF_LOG" >&2 || true
        exit 1
      fi
    fi
    sleep 1
  done
  red "[wait] 超时"
  tail -n 50 "$WAF_LOG" >&2 || true
  exit 1
}

stop_waf_expert_stack() {
  stop_pid_file "$WAF_PID_FILE" "nerve-waf-api"
  stop_pid_file "$EDGE_PID_FILE" "nerve-waf-edge"
  free_listen_port "${EDGE_PORT:-8080}"
  stop_pid_file "$CONSOLE_PID_FILE" "console-react"
  free_listen_port "${CONSOLE_PORT:-5177}"
  stop_pid_file "$LICENSE_PID_FILE" "mock-license"
  rm -f "$RUNTIME_ENV"
  green "已停止 Nerve-waf + WAF Edge + Expert + Console 测试栈"
}

print_waf_expert_status() {
  load_runtime_env
  bold "Nerve-waf Expert 状态"
  echo "  API:     ${API:-http://127.0.0.1:${PORT:-8782}}"
  echo "  Edge:    ${EDGE_URL:-http://127.0.0.1:${EDGE_PORT:-8080}}"
  echo "  Console: ${CONSOLE_URL:-http://127.0.0.1:${CONSOLE_PORT:-5177}}"
  echo "  模式:    ${MODE:-?}"
  echo "  Pack:    ${NERVE_EXPERT_PACK:-?}"
  if [[ -f "$WAF_PID_FILE" ]]; then
    local pid
    pid="$(cat "$WAF_PID_FILE")"
    if kill -0 "$pid" 2>/dev/null; then
      green "  WAF:     running (pid $pid)"
    else
      red "  WAF:     pid 文件存在但进程不在 (pid $pid)"
    fi
  else
    yellow "  WAF:     未启动"
  fi
  if [[ -f "$EDGE_PID_FILE" ]]; then
    local epid
    epid="$(cat "$EDGE_PID_FILE")"
    if kill -0 "$epid" 2>/dev/null; then
      green "  Edge:    running (pid $epid)"
    else
      red "  Edge:    pid 文件存在但进程不在 (pid $epid)"
    fi
  else
    yellow "  Edge:    未启动"
  fi
  if [[ -f "$LICENSE_PID_FILE" ]]; then
    local lpid
    lpid="$(cat "$LICENSE_PID_FILE")"
    if kill -0 "$lpid" 2>/dev/null; then
      green "  License mock: running (pid $lpid)"
    fi
  fi
  if [[ -f "$CONSOLE_PID_FILE" ]]; then
    local cpid
    cpid="$(cat "$CONSOLE_PID_FILE")"
    if kill -0 "$cpid" 2>/dev/null; then
      green "  Console: running (pid $cpid)"
    else
      red "  Console: pid 文件存在但进程不在 (pid $cpid)"
    fi
  else
    yellow "  Console: 未启动"
  fi
  echo "  日志:    WAF ${WAF_LOG:-$ROOT/logs/nerve-waf-expert.log}"
  echo "           Edge ${EDGE_LOG:-$ROOT/logs/nerve-waf-edge.log}"
  echo "           Console ${CONSOLE_LOG:-$ROOT/logs/nerve-waf-expert-console.log}"
}
