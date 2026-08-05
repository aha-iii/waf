#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -euo pipefail
BASE="${1:-http://127.0.0.1:8792}"
curl -sS --max-time 5 "${BASE}/healthz"
echo
curl -sS --max-time 30 -X POST "${BASE}/api/inspect" \
  -H "Content-Type: application/json" \
  -d '{"method":"GET","host":"x.com","uri":"/?id=1'\'' OR '\''1'\''='\''1","headers":{},"body":"","src_ip":"1.2.3.4","user_agent":"sqlmap/1.0"}'
echo
