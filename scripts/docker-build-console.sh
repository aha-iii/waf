#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

MONO="$(cd "$(dirname "$0")/../../.." && pwd)"
SRC="$MONO/Nerve-waf/Nerve-waf/apps/console-react"
DIST="$SRC/dist"

docker run --rm \
  -v "$MONO:/repo:ro" \
  -v "$DIST:/out" \
  node:20-bookworm \
  bash -lc '
    set -euo pipefail
    rm -rf /work
    mkdir -p /work/Nerve-waf/Nerve-waf/apps
    cp -a /repo/Nerve-ui /work/Nerve-ui
    cp -a /repo/Nerve-waf/Nerve-waf/apps/console-react /work/Nerve-waf/Nerve-waf/apps/console-react
    cd /work/Nerve-waf/Nerve-waf/apps/console-react
    npm install --include=dev
    npm run typecheck
    npm run build
    mkdir -p /out
    find /out -mindepth 1 -maxdepth 1 -exec rm -rf {} +
    cp -a dist/. /out/
    echo BUILD_OK
  '
