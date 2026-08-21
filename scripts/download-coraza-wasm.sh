#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/wasm/coraza-proxy-wasm.wasm"
VER="${CORAZA_PROXY_WASM_VERSION:-0.6.0}"
URL="https://github.com/corazawaf/coraza-proxy-wasm/releases/download/${VER}/coraza-proxy-wasm-${VER}.zip"

mkdir -p "$(dirname "$OUT")"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

echo "Downloading coraza-proxy-wasm ${VER}..."
curl -fsSL -o "$tmpdir/coraza.zip" "$URL"
python3 -m zipfile -e "$tmpdir/coraza.zip" "$tmpdir/extract"
wasm="$(find "$tmpdir/extract" -name '*.wasm' | head -1)"
if [ -z "$wasm" ]; then
  echo "error: no .wasm in release zip" >&2
  exit 1
fi
cp "$wasm" "$OUT"
echo "Installed: $OUT ($(wc -c < "$OUT") bytes)"
