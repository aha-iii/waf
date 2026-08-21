#!/usr/bin/env bash
# 按环境变量克隆同级依赖（未设置则只打印说明）。
#   NERVE_ANTIBODY_GIT=https://...git
#   NERVE_ALERT_GIT=https://...git
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WAF_PARENT="$(cd "$ROOT/.." && pwd)"
ECO_ROOT="$(cd "$ROOT/../.." && pwd)"

clone_if() {
  local url="${1:-}"
  local dest="$2"
  local name="$3"
  if [[ -z "$url" ]]; then
    echo "[skip] $name: set env to clone (dest would be $dest)"
    return 0
  fi
  if [[ -d "$dest/.git" || -f "$dest/Cargo.toml" || -d "$dest/crates" ]]; then
    echo "[exists] $name → $dest"
    return 0
  fi
  mkdir -p "$(dirname "$dest")"
  echo "[clone] $name ← $url"
  git clone --depth 1 "$url" "$dest"
}

echo "fetch-sibling-deps (eco root: $ECO_ROOT)"
clone_if "${NERVE_ANTIBODY_GIT:-}" "$WAF_PARENT/Nerve-antibody" "Nerve-antibody"
clone_if "${NERVE_ALERT_GIT:-}" "$ECO_ROOT/Nerve-alert" "Nerve-alert"

bash "$ROOT/scripts/check-sibling-deps.sh"
