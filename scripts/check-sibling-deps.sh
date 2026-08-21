#!/usr/bin/env bash
# 校验 Nerve WAF 构建所需的同级 path 依赖是否存在。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAIL=0

check() {
  local label="$1"
  local path="$2"
  if [[ -e "$path" ]]; then
    echo "[ok] $label → $path"
  else
    echo "[missing] $label → $path"
    FAIL=1
  fi
}

echo "Nerve WAF sibling deps (relative to $ROOT)"
check "Nerve-antibody (expert)" \
  "$ROOT/../Nerve-antibody/crates/integration/nerve-antibody"
check "Nerve-antibody metrics" \
  "$ROOT/../Nerve-antibody/crates/engine/facade/nerve-antibody-metrics"
check "Nerve-alert client" \
  "$ROOT/../../Nerve-alert/crates/nerve-alert-client"
check "Nerve-alert sdk" \
  "$ROOT/../../Nerve-alert/crates/nerve-alert-sdk"

if [[ "$FAIL" -ne 0 ]]; then
  echo
  echo "Layout expected:"
  echo "  <root>/Nerve-alert/"
  echo "  <root>/Nerve-waf/Nerve-waf/   # this repo"
  echo "  <root>/Nerve-waf/Nerve-antibody/"
  echo "See docs/DEPENDENCIES.md or: bash scripts/fetch-sibling-deps.sh"
  exit 1
fi

echo "All sibling paths present."
