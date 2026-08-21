#!/usr/bin/env bash
# WAF AI 冒烟：API 路由 +（可选）schema + Engram
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_PORT="${WAF_EDGE_STACK_API_PORT:-8781}"
KEY="${NERVE_API_KEY:-dev-nerve-key}"
BASE="http://127.0.0.1:${API_PORT}"

if [[ -f "$ROOT/.env" ]]; then
  set -a
  # shellcheck disable=SC1090
  source <(sed 's/\r$//' "$ROOT/.env")
  set +a
  KEY="${NERVE_API_KEY:-$KEY}"
fi

echo "=== WAF AI smoke $BASE ==="
bash "$ROOT/scripts/verify-waf-ai-api.sh"

code="$(curl -s -o /tmp/waf-ai-health.json -w '%{http_code}' -H "x-api-key: $KEY" "$BASE/api/ai/health")"
echo "GET /api/ai/health → HTTP $code"
if [[ "$code" != "200" ]]; then
  cat /tmp/waf-ai-health.json 2>/dev/null || true
  exit 1
fi
grep -q '"ok"' /tmp/waf-ai-health.json || { echo "invalid ai health body"; exit 1; }

if [[ -n "${DATABASE_URL:-}" ]]; then
  bash "$ROOT/scripts/check-ai-schema.sh"
fi

echo "=== smoke OK ==="
