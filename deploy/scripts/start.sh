#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -euo pipefail
DEPLOY="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DEPLOY"
TAR="../dist/nerve-waf-api-1.0.0.tar"
if ! docker image inspect nerve/nerve-waf-api:1.0.0 >/dev/null 2>&1; then
  [[ -f "$TAR" ]] || { echo "missing $TAR — place tar or build image first"; exit 1; }
  docker load -i "$TAR"
fi
docker compose up -d
echo "API/Console: http://127.0.0.1:8781/api/health"
echo "Edge:        http://127.0.0.1:8080/health"
