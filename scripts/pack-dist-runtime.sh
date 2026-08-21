#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 将 Linux 二进制 + Expert baseline + Console + 官方站静态资源打入 dist/runtime/。
#
#   cd Nerve-waf
#   bash scripts/pack-dist-runtime.sh              # 仅打包（需已 release 二进制 + console/official dist）
#   bash scripts/pack-dist-runtime.sh --build      # cargo build + console/website vite build + 打包
#
# --build 时 console/website：优先 WSL nvm Node>=18；否则 docker node:20

WAF_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$WAF_ROOT/.." && pwd)"
MONO_ROOT="$(cd "$REPO_ROOT/.." && pwd)"
OUT="$WAF_ROOT/dist/runtime"
BIN_API="$WAF_ROOT/target/release/nerve-waf-api"
BIN_EDGE="$WAF_ROOT/target/release/nerve-waf-edge"
BIN_CONSOLE="$WAF_ROOT/target/release/nerve-waf-console"
BIN_ALERT="$MONO_ROOT/Nerve-alert/target/release/nerve-alert-api"
PACK_SRC="$REPO_ROOT/Nerve-antibody/packs/baseline"
CONSOLE_DIR="$WAF_ROOT/apps/console-react"
CONSOLE_DIST="$CONSOLE_DIR/dist"
WEBSITE_DIR="$MONO_ROOT/Nerve-website"
WEBSITE_DIST="$WEBSITE_DIR/dist"

DO_BUILD=false
PACK_TEST=false
for arg in "$@"; do
  case "$arg" in
    --build) DO_BUILD=true ;;
    --test) PACK_TEST=true ;;
    -h|--help)
      echo "usage: $0 [--build] [--test]"
      echo "  --test  mars-test 发布：剔除 ssrf 规则（Engram 默认一并打包）"
      exit 0
      ;;
    *) echo "unknown arg: $arg" >&2; exit 1 ;;
  esac
done

# shellcheck source=lib/console-build.sh
source "$(cd "$(dirname "$0")" && pwd)/lib/console-build.sh"
# shellcheck source=lib/website-build.sh
source "$(cd "$(dirname "$0")" && pwd)/lib/website-build.sh"
# shellcheck source=lib/pack-engram-runtime.sh
source "$(cd "$(dirname "$0")" && pwd)/lib/pack-engram-runtime.sh"
if [[ "$PACK_TEST" == "true" ]]; then
  # shellcheck source=lib/pack-baseline-test-filter.sh
  source "$(cd "$(dirname "$0")" && pwd)/lib/pack-baseline-test-filter.sh"
fi
console_build_init "$WAF_ROOT"
website_build_init "$MONO_ROOT"

if [[ "$DO_BUILD" == "true" ]]; then
  FEATURES="${NERVE_NATIVE_BUILD_FEATURES:-}"
  if [[ -z "$FEATURES" ]]; then
    FEATURES="accel-hyperscan"
    if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists libhs 2>/dev/null; then
      FEATURES="accel-hyperscan-ffi"
    fi
  fi
  # --bin 是全局过滤器：只列 api/console 时不会产出 nerve-waf-edge，必须显式带上。
  if [[ -n "$FEATURES" ]]; then
    echo ">> cargo build --release -p nerve-waf-api --bin nerve-waf-api --bin nerve-waf-console -p nerve-waf-edge --bin nerve-waf-edge --features $FEATURES"
    (cd "$WAF_ROOT" && cargo build --release -p nerve-waf-api --bin nerve-waf-api --bin nerve-waf-console -p nerve-waf-edge --bin nerve-waf-edge --features "$FEATURES")
  else
    echo ">> cargo build --release -p nerve-waf-api --bin nerve-waf-api --bin nerve-waf-console -p nerve-waf-edge --bin nerve-waf-edge"
    (cd "$WAF_ROOT" && cargo build --release -p nerve-waf-api --bin nerve-waf-api --bin nerve-waf-console -p nerve-waf-edge --bin nerve-waf-edge)
  fi
  echo ">> cargo build --release -p nerve-alert-api (Nerve-alert)"
  (cd "$MONO_ROOT/Nerve-alert" && cargo build --release -p nerve-alert-api)
  echo ">> build console-react"
  build_console_react
  echo ">> build Nerve-website"
  if [[ "$PACK_TEST" == "true" ]]; then
    build_nerve_website test
  else
    build_nerve_website production
  fi
fi

for f in "$BIN_API" "$BIN_EDGE" "$BIN_CONSOLE" "$BIN_ALERT"; do
  if [[ ! -f "$f" ]]; then
    echo "missing $f — run: cargo build --release (waf-api/edge + Nerve-alert nerve-alert-api)" >&2
    exit 1
  fi
  if ! file "$f" | grep -q 'ELF.*x86-64'; then
    echo "not Linux amd64 ELF: $f ($(file -b "$f"))" >&2
    exit 1
  fi
done

if [[ ! -d "$PACK_SRC" ]]; then
  echo "missing baseline pack: $PACK_SRC" >&2
  exit 1
fi

if [[ ! -f "$CONSOLE_DIST/index.html" ]]; then
  echo "missing console dist: $CONSOLE_DIST/index.html — run with --build or: docker run --rm -v \$MONO_ROOT:/src -w /src/Nerve-waf/Nerve-waf/apps/console-react node:20-bookworm bash -lc 'npm ci && npx vite build'" >&2
  exit 1
fi

if [[ ! -f "$WEBSITE_DIST/index.html" ]]; then
  echo "missing official dist: $WEBSITE_DIST/index.html — run with --build or: docker run --rm -v \$MONO_ROOT:/src -w /src/Nerve-website node:20-bookworm bash -lc 'npm ci && npm run build'" >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/baseline" "$OUT/console" "$OUT/official"

echo ">> copy binaries"
cp -f "$BIN_API" "$OUT/nerve-waf-api"
cp -f "$BIN_EDGE" "$OUT/nerve-waf-edge"
cp -f "$BIN_CONSOLE" "$OUT/nerve-waf-console"
cp -f "$BIN_ALERT" "$OUT/nerve-alert-api"
chmod 755 "$OUT/nerve-waf-api" "$OUT/nerve-waf-edge" "$OUT/nerve-waf-console" "$OUT/nerve-alert-api"

echo ">> copy baseline pack"
cp -a "$PACK_SRC/." "$OUT/baseline/"
if [[ "$PACK_TEST" == "true" ]]; then
  echo ">> mars-test: filter baseline rules"
  pack_baseline_filter_for_test "$OUT/baseline" ssrf-internal-ip ssrf-loopback
fi
pack_engram_runtime "$WAF_ROOT" "$MONO_ROOT" "$OUT"

echo ">> copy console dist"
cp -a "$CONSOLE_DIST/." "$OUT/console/"

echo ">> copy official website dist"
cp -a "$WEBSITE_DIST/." "$OUT/official/"

# 预置 OWASP CRS（无 git）：生产「更新最新规则集」在外网不可达时回退导入
CRS_VENDOR_SRC=""
for cand in \
  "$WAF_ROOT/vendor/coreruleset" \
  "$WAF_ROOT/data/nerve-waf-expert/vendor/coreruleset"; do
  if [[ -d "$cand/rules" ]]; then
    CRS_VENDOR_SRC="$cand"
    break
  fi
done
if [[ -n "$CRS_VENDOR_SRC" ]]; then
  echo ">> copy bundled CRS vendor from $CRS_VENDOR_SRC"
  mkdir -p "$OUT/vendor/coreruleset"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete --exclude='.git' "$CRS_VENDOR_SRC/" "$OUT/vendor/coreruleset/"
  else
    rm -rf "$OUT/vendor/coreruleset"
    mkdir -p "$OUT/vendor/coreruleset"
    tar -C "$CRS_VENDOR_SRC" --exclude='.git' -cf - . | tar -C "$OUT/vendor/coreruleset" -xf -
  fi
else
  echo "WARN: no bundled CRS vendor (vendor/coreruleset) — online tarball download required" >&2
fi

GIT_SHA="$(git -C "$WAF_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
cat > "$OUT/MANIFEST.txt" <<EOF
packed_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
git_sha=$GIT_SHA
pack_profile=$([[ "$PACK_TEST" == "true" ]] && echo mars-test || echo default)
excluded_rules=$([[ "$PACK_TEST" == "true" ]] && echo "ssrf-internal-ip,ssrf-loopback" || echo "")
nerve-waf-api=$(sha256sum "$OUT/nerve-waf-api" | awk '{print $1}')
nerve-waf-edge=$(sha256sum "$OUT/nerve-waf-edge" | awk '{print $1}')
nerve-waf-console=$(sha256sum "$OUT/nerve-waf-console" | awk '{print $1}')
nerve-alert-api=$(sha256sum "$OUT/nerve-alert-api" | awk '{print $1}')
baseline=$(find "$OUT/baseline" -type f | wc -l) files
console=$(find "$OUT/console" -type f | wc -l) files
official=$(find "$OUT/official" -type f | wc -l) files
crs_vendor=$([[ -d "$OUT/vendor/coreruleset/rules" ]] && echo "yes" || echo "no")
engram=$([[ -d "$OUT/engram/gateway-dist/src" ]] && echo "yes" || echo "no")
EOF

echo ">> done: $OUT"
ls -la "$OUT"
echo "copy to test host: dist/runtime/ + dist/nerve-waf-api-*.tar + deploy/mars-test/"
