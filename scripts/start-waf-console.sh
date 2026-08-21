#!/usr/bin/env bash
# 启动 nerve-waf-console（静态 UI + /api 反代到 nerve-waf-api）
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source <(sed 's/\r$//' .env)
  set +a
  echo "[start-waf-console] loaded .env"
fi

# shellcheck source=lib/console-build.sh
source "$SCRIPT_DIR/lib/console-build.sh"
console_build_init "$ROOT"
ensure_console_dist
export WAF_CONSOLE_DIR="$CONSOLE_DIST"

export WAF_API_UPSTREAM="${WAF_API_UPSTREAM:-http://127.0.0.1:${PORT:-8781}}"
export WAF_CONSOLE_PORT="${WAF_CONSOLE_PORT:-${CONSOLE_PORT:-8782}}"

BIN="${ROOT}/target/release/nerve-waf-console"
if [[ ! -x "$BIN" ]]; then
  echo "[start-waf-console] building release nerve-waf-console..."
  CARGO_INCREMENTAL=0 RUSTFLAGS='-A dead_code' cargo build --release -p nerve-waf-api --bin nerve-waf-console
fi

echo "[start-waf-console] Console UI: http://127.0.0.1:${WAF_CONSOLE_PORT}/"
echo "[start-waf-console] API upstream: ${WAF_API_UPSTREAM}"

exec "$BIN"
