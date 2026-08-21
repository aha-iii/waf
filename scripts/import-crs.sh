#!/usr/bin/env bash
# 下载/更新 OWASP CRS 并导入为 Nerve WAF JSON 规则包
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CRS_VERSION="${CRS_VERSION:-v4.25.0}"
PARANOIA_MAX="${PARANOIA_MAX:-2}"
VENDOR_DIR="$ROOT/vendor/coreruleset"
OUT_DIR="${NERVE_RULES_DIR:-$(cd "$ROOT/.." && pwd)/rules/crs}"
WAF_API="${WAF_API:-http://127.0.0.1:8780}"

echo "==> OWASP CRS import ($CRS_VERSION, paranoia<=$PARANOIA_MAX)"
echo "    vendor: $VENDOR_DIR"
echo "    output: $OUT_DIR"

if [[ ! -d "$VENDOR_DIR/rules" ]]; then
  mkdir -p "$ROOT/vendor"
  if [[ -d "$VENDOR_DIR/.git" ]]; then
    git -C "$VENDOR_DIR" fetch --tags
    git -C "$VENDOR_DIR" checkout "$CRS_VERSION"
  else
    git clone --depth 1 --branch "$CRS_VERSION" \
      https://github.com/coreruleset/coreruleset.git "$VENDOR_DIR"
  fi
else
  echo "    using existing $VENDOR_DIR (set CRS_VERSION to change tag)"
fi

cargo build --release -p nerve-waf-crs-import

"$ROOT/target/release/nerve-waf-crs-import" \
  --crs-dir "$VENDOR_DIR" \
  --out-dir "$OUT_DIR" \
  --version "${CRS_VERSION#v}" \
  --paranoia-max "$PARANOIA_MAX" \
  --clean

echo ""
echo "==> Reload WAF rules"
if curl -sf -X POST "$WAF_API/api/rules/packs" >/dev/null 2>&1; then
  curl -s -X POST "$WAF_API/api/rules/packs" | head -c 500
  echo ""
else
  echo "    WAF API not reachable at $WAF_API — run manually:"
  echo "    curl -X POST $WAF_API/api/rules/packs"
fi

echo ""
echo "Done. Check rule count:"
echo "  curl -s $WAF_API/api/rules/packs | jq '.total_rules, .packs[].pack_id'"
