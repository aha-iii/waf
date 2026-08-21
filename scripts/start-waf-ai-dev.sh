#!/usr/bin/env bash
# WAF AI 开发栈 — 一键启动（仅 Nerve-waf 范围）
#
# 默认：WAF API + Edge + Console（:5177 含 AI 页面）
# 可选：Engram 推理、Alert posture 联动（均通过 WAF API 消费）
#
# 用法（WSL）:
#   cd Nerve-waf/Nerve-waf
#   bash scripts/start-waf-ai-dev.sh              # 仅 WAF 三件套
#   bash scripts/start-waf-ai-dev.sh --full       # + Engram + Alert API
#   bash scripts/start-waf-ai-dev.sh --with-engram
#   bash scripts/start-waf-ai-dev.sh --build       # 改 AI 源码后必须重编 API
#   bash scripts/verify-waf-ai-api.sh              # 单独探针
#
# 不含：Platform / Control / Agent（属 monorepo 其他产品，与当前 WAF Console AI 无硬依赖）

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MONO="$(cd "$ROOT/../.." && pwd)"
ALERT_ROOT="$MONO/Nerve-alert"
LLM_ROOT="$MONO/Nerve-llm"
STATE_DIR="$ROOT/data/waf-ai-dev"
mkdir -p "$STATE_DIR" "$ROOT/logs"

ENGRAM_PID="$STATE_DIR/engram.pid"
ALERT_PID="$STATE_DIR/alert-api.pid"
ENGRAM_LOG="$ROOT/logs/waf-ai-engram.log"
ALERT_LOG="$ROOT/logs/waf-ai-alert.log"

WITH_ENGRAM="${WAF_AI_WITH_ENGRAM:-0}"
WITH_ALERT="${WAF_AI_WITH_ALERT:-0}"
CMD="start"
WAF_ARGS=()

green() { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
red() { printf '\033[31m%s\033[0m\n' "$*"; }
bold() { printf '\033[1m%s\033[0m\n' "$*"; }

usage() {
  sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

load_waf_env() {
  if [[ -f "$ROOT/.env" ]]; then
    set -a
    # shellcheck disable=SC1090
    source <(sed 's/\r$//' "$ROOT/.env")
    set +a
  fi
  export NERVE_API_KEY="${NERVE_API_KEY:-dev-nerve-key}"
  export NERVE_ENGRAM_URL="${NERVE_ENGRAM_URL:-http://127.0.0.1:8795}"
}

stop_pid() {
  local file="$1" label="$2"
  if [[ -f "$file" ]]; then
    local pid
    pid="$(cat "$file" 2>/dev/null || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.3
      kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
      yellow "[stop] $label (pid $pid)"
    fi
    rm -f "$file"
  fi
}

wait_http() {
  local url="$1" label="$2" tries="${3:-40}"
  for _ in $(seq 1 "$tries"); do
    if curl -sf --max-time 2 "$url" >/dev/null 2>&1; then
      green "[ready] $label"
      return 0
    fi
    sleep 1
  done
  red "[timeout] $label → $url"
  return 1
}

start_waf_stack() {
  yellow "[waf] API + Edge + Console"
  bash "$ROOT/scripts/start-waf-edge-stack.sh" start "${WAF_ARGS[@]}"
}

stop_waf_stack() {
  bash "$ROOT/scripts/start-waf-edge-stack.sh" stop 2>/dev/null || true
}

start_engram() {
  command -v npm >/dev/null 2>&1 || { yellow "[engram] 无 npm，跳过"; return 0; }
  stop_pid "$ENGRAM_PID" "engram-gateway"
  ensure_engram_artifact
  yellow "[engram] $NERVE_ENGRAM_URL（可选，持续学习/Engram 推理）"
  cd "$LLM_ROOT"
  [[ -d node_modules ]] || npm install
  nohup env ENGRAM_PORT=8795 ENGRAM_MODEL_ID=engram-waf-v1 \
    ENGRAM_ARTIFACT_PATH="${ENGRAM_ARTIFACT_PATH:-$LLM_ROOT/training/data/engram-linear-v1.json}" \
    npm run gateway:dev >> "$ENGRAM_LOG" 2>&1 &
  echo $! > "$ENGRAM_PID"
  if ! wait_http "${NERVE_ENGRAM_URL%/}/health" "Engram Gateway" 90; then
    yellow "[engram] 启动失败，不影响 WAF Console AI 页面；日志: $ENGRAM_LOG"
    tail -n 20 "$ENGRAM_LOG" 2>/dev/null || true
    return 0
  fi
}

ensure_engram_artifact() {
  local artifact="${ENGRAM_ARTIFACT_PATH:-$LLM_ROOT/training/data/engram-linear-v1.json}"
  if [[ -f "$artifact" ]]; then
    return 0
  fi
  yellow "[engram] 缺少 artifact，生成 dev 占位: $artifact"
  mkdir -p "$(dirname "$artifact")"
  cat > "$artifact" <<'EOF'
{
  "schema": "engram-linear-v1",
  "threshold": 0.5,
  "patterns": [],
  "category_weights": {},
  "stats": { "records": 0, "patterns": 0, "categories": 0, "note": "dev bootstrap" }
}
EOF
}

verify_ai_routes() {
  yellow "[verify] 检查 AI API 路由…"
  if bash "$ROOT/scripts/verify-waf-ai-api.sh"; then
    return 0
  fi
  red "[verify] Console「防护巡检/策略预演」等会 404 或空白，请执行:"
  echo "  bash scripts/start-waf-ai-dev.sh stop && bash scripts/start-waf-ai-dev.sh --build"
  return 1
}

start_alert_api() {
  local bin="$ALERT_ROOT/target/release/nerve-alert-api"
  if [[ ! -x "$bin" ]]; then
    yellow "[alert] 编译 nerve-alert-api..."
    (cd "$ALERT_ROOT" && cargo build --release -p nerve-alert-api)
  fi
  stop_pid "$ALERT_PID" "nerve-alert-api"
  yellow "[alert] :8790（定时拉 WAF posture，可选）"
  nohup env PORT=8790 NERVE_ALERT_DATA="$ALERT_ROOT/data" \
    DATABASE_URL="${DATABASE_URL:-}" \
    NERVE_API_KEY="$NERVE_API_KEY" \
    "$bin" >> "$ALERT_LOG" 2>&1 &
  echo $! > "$ALERT_PID"
  wait_http "http://127.0.0.1:8790/health" "Alert API"
}

cmd_start() {
  load_waf_env
  start_waf_stack

  if [[ "$WITH_ENGRAM" == "1" ]]; then
    start_engram
  fi
  if [[ "$WITH_ALERT" == "1" ]]; then
    start_alert_api || true
  fi

  verify_ai_routes || true

  if [[ -n "${DATABASE_URL:-}" ]]; then
    yellow "[schema] 检查 AI 迁移表…"
    bash "$ROOT/scripts/check-ai-schema.sh" || yellow "[schema] 未就绪，请 bash scripts/apply-migrations.sh"
  fi

  bold ""
  bold "WAF AI 开发栈已启动"
  echo "  Console:  http://127.0.0.1:5177  （admin/admin）"
  echo "  WAF API:  http://127.0.0.1:8781"
  echo "  Edge:     http://127.0.0.1:8080"
  [[ "$WITH_ENGRAM" == "1" ]] && echo "  Engram:   ${NERVE_ENGRAM_URL}/health"
  [[ "$WITH_ALERT" == "1" ]] && echo "  Alert:    http://127.0.0.1:8790/health"
  echo ""
  echo "  AI 页面均在 :5177 → 防护巡检 / 策略预演 / 持续学习 / 安全图谱 / Copilot"
  echo "  数据库:   bash scripts/apply-migrations.sh  （PostgreSQL 能力需 DATABASE_URL）"
  echo "  冒烟:     bash scripts/e2e-waf-ai-smoke.sh"
  echo "  停止:     bash scripts/start-waf-ai-dev.sh stop"
  bold ""
}

cmd_stop() {
  stop_pid "$ALERT_PID" "nerve-alert-api"
  stop_pid "$ENGRAM_PID" "engram-gateway"
  stop_waf_stack
  green "WAF AI 开发栈已停止"
}

cmd_status() {
  bash "$ROOT/scripts/start-waf-edge-stack.sh" status 2>/dev/null || true
  for pair in "$ENGRAM_PID:Engram" "$ALERT_PID:Alert API"; do
    local file="${pair%%:*}" label="${pair##*:}"
    if [[ -f "$file" ]] && kill -0 "$(cat "$file")" 2>/dev/null; then
      green "  $label: running (pid $(cat "$file"))"
    else
      yellow "  $label: 未启动"
    fi
  done
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --full) WITH_ENGRAM=1; WITH_ALERT=1; shift ;;
    --with-engram) WITH_ENGRAM=1; shift ;;
    --with-alert) WITH_ALERT=1; shift ;;
    --skip-build|--build|--no-console)
      WAF_ARGS+=("$1")
      shift
      ;;
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
