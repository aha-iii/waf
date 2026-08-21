#!/usr/bin/env bash
# 启动 Nerve-waf API（L7 inspect / WAF-L3 / 事件）— 默认端口见 config.yml
# Console 独立进程：bash scripts/start-waf-console.sh（或 Vite dev :5177）
# 拓扑 / Docker 库存 / 组网页 API 请用仓库根: bash scripts/start-nerve-control.sh (:8780)
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CONFIG="${WAF_CONFIG:-${ROOT}/config.yml}"

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source <(sed 's/\r$//' .env)
  set +a
  echo "[start-api] loaded .env (overrides config.yml when set)"
else
  echo "[start-api] using config.yml: ${CONFIG}"
fi

export NERVE_WAF_DATA="${NERVE_WAF_DATA:-./data}"
export NERVE_RULES_DIR="${NERVE_RULES_DIR:-$(cd "$ROOT/.." && pwd)/rules/crs}"
mkdir -p "${ROOT}/logs"

# 默认自研 Console 登录（与 start-nerve-waf-expert.sh 一致）
export NERVE_AUTH_ENABLED="${NERVE_AUTH_ENABLED:-1}"
export NERVE_AUTH_MODE="${NERVE_AUTH_MODE:-local}"
export KEYCLOAK_ENABLED="${KEYCLOAK_ENABLED:-0}"
export NERVE_API_KEY="${NERVE_API_KEY:-dev-nerve-key}"

# 分离模式：API 不托管 Console 静态；本地联调可 WAF_CONSOLE_ATTACH=1
export WAF_CONSOLE_ATTACH="${WAF_CONSOLE_ATTACH:-0}"

API_PORT="${PORT:-8781}"

if [[ -n "${DATABASE_URL:-}" ]]; then
  echo "[start-api] DATABASE_URL -> PostgreSQL ($(echo "$DATABASE_URL" | sed 's/:.*@/:***@/'))"
else
  echo "[start-api] DATABASE_URL unset -> file store or config.yml database.url"
fi

if [[ "$WAF_CONSOLE_ATTACH" == "1" ]]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  # shellcheck source=lib/console-build.sh
  source "$SCRIPT_DIR/lib/console-build.sh"
  console_build_init "$ROOT"
  ensure_console_dist
  export WAF_CONSOLE_DIR="$CONSOLE_DIST"
  echo "[start-api] WAF_CONSOLE_ATTACH=1: Console UI on http://127.0.0.1:${API_PORT}/"
else
  echo "[start-api] API only (Console: bash scripts/start-waf-console.sh or Vite :5177)"
fi

echo "[start-api] API: http://127.0.0.1:${API_PORT}/api/health"
echo "[start-api] 登录: waf_users 表（首次启动自动 bootstrap admin/admin）"

BIN="${ROOT}/target/release/nerve-waf-api"
if [[ ! -x "$BIN" ]]; then
  echo "[start-api] building release nerve-waf-api..."
  CARGO_INCREMENTAL=0 RUSTFLAGS='-A dead_code' cargo build --release -p nerve-waf-api
fi

exec "$BIN" --config "$CONFIG" "$@"
