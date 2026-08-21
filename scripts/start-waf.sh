#!/usr/bin/env bash
# Nerve WAF 控制面 — Control API :8780
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "============================================"
echo " Nerve WAF — Control API (:8781 via start-api)"
echo " 职责: OWASP 模块 / 策略 / /api/inspect / 事件 / WAF-L3"
echo "============================================"

exec "$ROOT/scripts/start-api.sh" "$@"
