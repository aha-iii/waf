#!/usr/bin/env bash
# 已迁移至 ../Nerve-gateway/scripts/start.sh
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
exec "$ROOT/../Nerve-gateway/scripts/start.sh" "$@"
