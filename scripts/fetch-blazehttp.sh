#!/usr/bin/env bash
# Clone chaitin/blazehttp and link into Nerve-waf-expert + Nerve-waf vendor/.
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CACHE="${BLAZEHTTP_CACHE:-/tmp/blazehttp}"
REPO_URL="${BLAZEHTTP_REPO:-https://github.com/chaitin/blazehttp.git}"

if [ ! -d "$CACHE/.git" ]; then
  echo "cloning $REPO_URL -> $CACHE"
  rm -rf "$CACHE"
  git clone --depth 1 "$REPO_URL" "$CACHE"
else
  echo "updating $CACHE"
  git -C "$CACHE" pull --ff-only || true
fi

mkdir -p "$ROOT/Nerve-waf-expert/vendor" "$ROOT/Nerve-waf/vendor"
ln -sfn "$CACHE" "$ROOT/Nerve-waf-expert/vendor/blazehttp"
ln -sfn "$CACHE" "$ROOT/Nerve-waf/vendor/blazehttp"

echo "linked:"
ls -la "$ROOT/Nerve-waf-expert/vendor/blazehttp" | head -1
ls -la "$ROOT/Nerve-waf/vendor/blazehttp" | head -1

if command -v go >/dev/null 2>&1; then
  echo "building blazehttp..."
  (cd "$CACHE" && go build -o "$CACHE/blazehttp" ./cmd/blazehttp)
  echo "binary: $CACHE/blazehttp"
else
  echo "go not found; skip build"
fi
