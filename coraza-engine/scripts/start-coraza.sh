#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail
export PATH="/snap/bin:/usr/local/go/bin:${HOME}/go/bin:${PATH:-}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export NERVE_CORAZA_LISTEN="${NERVE_CORAZA_LISTEN:-127.0.0.1:8791}"
echo "nerve-waf-coraza → http://${NERVE_CORAZA_LISTEN}"
exec go run ./cmd/nerve-waf-coraza -listen "${NERVE_CORAZA_LISTEN}"
