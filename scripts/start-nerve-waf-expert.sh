#!/usr/bin/env bash
# 启动 Nerve-waf API + Native Expert 引擎 + WAF Console（console-react，后台）
#
# 用法（WSL / Linux）:
#   cd Nerve-waf
#   bash scripts/start-nerve-waf-expert.sh              # dev 明文 pack
#   bash scripts/start-nerve-waf-expert.sh --sealed       # 加密包 + Mock License
#   bash scripts/start-nerve-waf-expert.sh --foreground   # 前台跑并 tail 日志
#   bash scripts/start-nerve-waf-expert.sh stop
#   bash scripts/start-nerve-waf-expert.sh status
#
# 环境变量:
#   NERVE_WAF_EXPERT_PORT=8782
#   NERVE_WAF_EXPERT_CONSOLE_PORT=5177
#   NERVE_WAF_EXPERT_PACK_NAME=baseline|demo
#   NERVE_WAF_EXPERT_SKIP_BUILD=1
#   NERVE_WAF_EXPERT_SKIP_CONSOLE=0   # 默认 1：Console 走 API 同端口静态页；设 0 另起 Vite :5177 热更新
#   NERVE_WAF_EXPERT_CONSOLE_DEV=1    # 同 SKIP_CONSOLE=0
#
# 启动后跑测试:
#   bash scripts/test-nerve-waf-expert.sh           # 冒烟 + WAF 评测（gotestwaf/blazehttp/waftester）
#   bash scripts/test-native-expert.sh              # 启动 + 测试一键

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/nerve-waf-expert-lib.sh
source "$SCRIPT_DIR/lib/nerve-waf-expert-lib.sh"
nerve_antibody_lib_init

CMD="start"
FOREGROUND=0

usage() {
  sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -h|--help) usage 0 ;;
      --sealed) MODE="sealed"; shift ;;
      --dev) MODE="dev"; shift ;;
      --foreground|-f) FOREGROUND=1; shift ;;
      --skip-build) SKIP_BUILD=1; shift ;;
      start|stop|status) CMD="$1"; shift ;;
      *) red "未知参数: $1"; usage 1 ;;
    esac
  done
}

cmd_start() {
  if [[ ! -d "$NATIVE_ROOT/packs/$PACK" && "$MODE" == "dev" ]]; then
    red "Native pack 不存在: $NATIVE_ROOT/packs/$PACK"
    exit 1
  fi

  if [[ "$MODE" == "sealed" ]]; then
    ensure_sealed_pack
    start_mock_license
  fi

  ensure_state_dir
  ensure_console_built
  export_expert_env
  build_waf_expert
  start_waf_expert
  wait_waf_api
  start_waf_edge
  wait_waf_edge
  if [[ "${NERVE_WAF_EXPERT_CONSOLE_DEV:-0}" == "1" ]]; then
    SKIP_CONSOLE=0
  fi
  start_waf_console
  write_runtime_env
  wait_waf_console

  bold ""
  bold "Nerve-waf + WAF Edge + Expert + Console 已启动"
  echo "  API:     $API"
  echo "  Console: http://127.0.0.1:$PORT/  (API 内置静态 UI，自研登录)"
  if [[ "$SKIP_CONSOLE" != "1" ]]; then
    echo "  Console dev: $CONSOLE_URL  (Vite 热更新，可选)"
  fi
  echo "  Edge:    $EDGE_URL  (形态 B 数据面，Host 路由 + 回源)"
  echo "  登录:    waf_users 表（bootstrap admin/admin）"
  echo "  模式:    $MODE"
  echo "  Pack:    $NERVE_EXPERT_PACK"
  echo "  日志:    WAF $WAF_LOG"
  echo "           Edge $EDGE_LOG"
  echo "           Console $CONSOLE_LOG"
  echo "  测试:     bash scripts/test-nerve-waf-expert.sh"
  echo "  停止:     bash scripts/start-nerve-waf-expert.sh stop"
  echo ""

  if [[ "$FOREGROUND" == "1" ]]; then
    yellow "前台模式: Ctrl+C 结束 tail（WAF 仍在后台）"
    tail -f "$WAF_LOG"
  fi
}

main() {
  parse_args "$@"
  case "$CMD" in
    start) cmd_start ;;
    stop) stop_waf_expert_stack ;;
    status) print_waf_expert_status ;;
    *) usage 1 ;;
  esac
}

main "$@"
