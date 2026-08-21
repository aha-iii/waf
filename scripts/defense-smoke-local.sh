#!/usr/bin/env bash
# 本机 WAF Edge 防护验收（WAFtester 全类型，与正式评测同口径）
#
# 默认拉取/使用 WAFtester payloads，按 payloads/*.json 类型汇总检出率与复盘报告。
# 硬限制：仅 127.0.0.1 / localhost / ::1。
#
# 前置:
#   bash scripts/start-waf-edge-stack.sh --no-console
#
# 用法（WSL）:
#   cd Nerve-waf/Nerve-waf
#   bash scripts/defense-smoke-local.sh
#   bash scripts/defense-smoke-local.sh --quick          # 每数据集最多 200 条（冒烟）
#   DATASETS=gotestwaf,waftester,blazehttp bash scripts/defense-smoke-local.sh
#   LIMIT=0 CONCURRENCY=20 bash scripts/defense-smoke-local.sh
#
# 环境变量:
#   EDGE_URL=http://127.0.0.1:8080
#   EDGE_HOST=test.local
#   DATASETS=waftester
#   LIMIT=0                 # 0=全集
#   CONCURRENCY=20
#   MIN_DETECTION=0.80
#   MAX_FP=0.10
#   NATIVE_ROOT=...         # 默认 ../Nerve-antibody

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

EDGE_URL="${EDGE_URL:-http://127.0.0.1:8080}"
EDGE_HOST="${EDGE_HOST:-test.local}"
DATASETS="${DATASETS:-waftester}"
LIMIT="${LIMIT:-0}"
CONCURRENCY="${CONCURRENCY:-20}"
TIMEOUT="${TIMEOUT:-30}"
MIN_DETECTION="${MIN_DETECTION:-0.80}"
MAX_FP="${MAX_FP:-0.10}"
NATIVE_ROOT="${NATIVE_ROOT:-$ROOT/../Nerve-antibody}"

QUICK=0
EXTRA_PY_ARGS=()

for arg in "$@"; do
  case "$arg" in
    --quick)
      QUICK=1
      LIMIT="${LIMIT:-200}"
      if [[ "$LIMIT" == "0" ]]; then
        LIMIT=200
      fi
      ;;
    --full|--)
      ;;
    -h|--help)
      sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      EXTRA_PY_ARGS+=("$arg")
      ;;
  esac
done

if [[ "$QUICK" == "1" && "$LIMIT" == "0" ]]; then
  LIMIT=200
fi

red() { printf '\033[31m%s\033[0m\n' "$*"; }
bold() { printf '\033[1m%s\033[0m\n' "$*"; }

assert_loopback() {
  local label="$1"
  local url="$2"
  local host
  host="$(printf '%s' "$url" | sed -E 's#^[a-zA-Z]+://([^/:]+).*#\1#')"
  case "$host" in
    127.0.0.1|localhost|::1|[::1]) ;;
    *)
      red "$label 拒绝非本机目标: $url"
      exit 2
      ;;
  esac
}

assert_loopback "EDGE_URL" "$EDGE_URL"

if [[ ! -d "$NATIVE_ROOT" ]]; then
  alt="$ROOT/../Nerve-waf-native"
  if [[ -d "$alt" ]]; then
    NATIVE_ROOT="$alt"
  fi
fi

# 缺数据集时先拉 waftester（其它集由 python ensure_dataset 再拉）
need_wt=0
case ",${DATASETS}," in
  *,waftester,*|*,wt,*) need_wt=1 ;;
esac
if [[ "$need_wt" == "1" ]]; then
  if [[ ! -d "${WAFTESTER_ROOT:-/tmp/waftester}/payloads" && ! -d "$NATIVE_ROOT/vendor/waftester/payloads" ]]; then
    bold "拉取 WAFtester 数据集..."
    if [[ -f "$NATIVE_ROOT/scripts/fetch-waftester.sh" ]]; then
      bash "$NATIVE_ROOT/scripts/fetch-waftester.sh"
    elif [[ -f "$ROOT/scripts/fetch-all-datasets.sh" ]]; then
      bash "$ROOT/scripts/fetch-all-datasets.sh" waftester || true
    fi
  fi
  if [[ -d /tmp/waftester/payloads ]]; then
    export WAFTESTER_ROOT="${WAFTESTER_ROOT:-/tmp/waftester}"
  elif [[ -d "$NATIVE_ROOT/vendor/waftester/payloads" ]]; then
    export WAFTESTER_ROOT="${WAFTESTER_ROOT:-$NATIVE_ROOT/vendor/waftester}"
  else
    red "WAFtester payloads 未就绪（请: bash $NATIVE_ROOT/scripts/fetch-waftester.sh）"
    exit 1
  fi
fi

PY="${PYTHON:-python3}"
if ! command -v "$PY" >/dev/null 2>&1; then
  PY=python
fi

export NATIVE_ROOT EDGE_URL EDGE_HOST DATASETS LIMIT CONCURRENCY TIMEOUT MIN_DETECTION MAX_FP

bold "============================================================"
bold " 本机 Edge 防护验收（WAFtester 口径）"
bold " EDGE=$EDGE_URL  Host=$EDGE_HOST"
bold " DATASETS=$DATASETS  LIMIT=$LIMIT  CONC=$CONCURRENCY"
bold "============================================================"
echo

exec "$PY" "$ROOT/scripts/defense-smoke-local.py" \
  --edge "$EDGE_URL" \
  --edge-host "$EDGE_HOST" \
  --datasets "$DATASETS" \
  --limit "$LIMIT" \
  --concurrency "$CONCURRENCY" \
  --timeout "$TIMEOUT" \
  --min-detection "$MIN_DETECTION" \
  --max-fp "$MAX_FP" \
  "${EXTRA_PY_ARGS[@]}"
