#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

DEPLOY="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DEPLOY"

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "[up-test] created .env from .env.example"
fi

TAG="${TAG:-1.0.0}"
IMAGE="nerve/nerve-waf-api:${TAG}"

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  TAR="${DEPLOY}/../dist/nerve-waf-api-${TAG}.tar"
  if [[ -f "$TAR" ]]; then
    echo "[up-test] loading image from $TAR"
    docker load -i "$TAR"
  else
    echo "[up-test] image $IMAGE not found — compose will build (slow) or run:"
    echo "  bash ${DEPLOY}/scripts/build-image.sh"
  fi
fi

PROFILE="${1:-}"
if [[ "$PROFILE" == "postgres" || "$PROFILE" == "--profile" ]]; then
  grep -q '^DATABASE_URL=' .env && grep -q 'postgres://' .env || \
    echo 'DATABASE_URL=postgres://nerve:nerve@postgres:5432/nerve' >> .env
  docker compose --profile postgres up -d
else
  docker compose up -d
fi

echo "[up-test] WAF API: http://127.0.0.1:${PORT:-8781}/api/health"
echo "[up-test] expert:  http://127.0.0.1:${PORT:-8781}/api/expert/health"
