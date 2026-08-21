#!/usr/bin/env bash
# 拉取 WAF 评测数据集（gotestwaf / blazehttp / waftester / crs-ftw）
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NATIVE="${NATIVE_ROOT:-$(cd "$SCRIPT_DIR/../Nerve-antibody" && pwd)}"
exec bash "$NATIVE/scripts/fetch-all-datasets.sh"
