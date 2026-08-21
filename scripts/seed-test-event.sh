#!/usr/bin/env bash
# 写入一条测试 SQL 注入事件到当前 event store（PostgreSQL 或文件）
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source <(sed 's/\r$//' .env)
  set +a
fi

API="${API_URL:-http://127.0.0.1:8780}"
echo "=== health ==="
curl -s "$API/api/health" | jq .
echo "=== inspect + persist ==="
curl -s -X POST "$API/api/inspect" \
  -H 'Content-Type: application/json' \
  -d '{"uri":"/search?q=1+or+1%3D1","persist":true}' | jq .
echo "=== events ==="
curl -s "$API/api/events?limit=3" | jq .
