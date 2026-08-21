#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 构建 nerve-waf-api + nerve-waf-edge，优先 accel-hyperscan-ffi（需 libhs），否则 accel-hyperscan（RegexSet）。
# 用法：bash scripts/build-with-hyperscan.sh [--release] [cargo 额外参数…]

WAF_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WAF_ROOT"

has_libhs=0
if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists libhs 2>/dev/null; then
  has_libhs=1
fi
if [[ "$has_libhs" -eq 0 ]]; then
  for cand in /usr/include/hs/hs.h /usr/local/include/hs/hs.h /usr/include/hs.h; do
    if [[ -f "$cand" ]]; then
      has_libhs=1
      break
    fi
  done
fi

FEATURES="accel-hyperscan"
if [[ "$has_libhs" -eq 1 ]]; then
  FEATURES="accel-hyperscan-ffi"
  echo "==> libhs detected; building with ${FEATURES}"
else
  echo "==> libhs not found; building with ${FEATURES} (RegexSet path)"
  echo "    install: sudo apt-get install -y libhyperscan-dev pkg-config"
fi

export NERVE_NATIVE_BUILD_FEATURES="$FEATURES"
exec cargo build -p nerve-waf-api -p nerve-waf-edge --features "$FEATURES" "$@"
