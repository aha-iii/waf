#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 对官网防护应用跑官方 GoTestWAF（评测用，非自研 payload）。
#
#   bash scripts/gotestwaf-www.sh
#   TARGET_URL=https://www.ahatec.com.cn/ bash scripts/gotestwaf-www.sh
#
# Nerve Edge 拦截码为 403；须显式传入 --blockStatusCodes，并关闭厂商指纹识别。

TARGET_URL="${TARGET_URL:-https://www.ahatec.com.cn/}"
IMAGE="${GOTESTWAF_IMAGE:-wallarm/gotestwaf:latest}"
REPORT_ROOT="${REPORT_ROOT:-$(pwd)/gotestwaf-reports}"
INCLUDE_PAYLOADS="${INCLUDE_PAYLOADS:-true}"
BLOCK_STATUS_CODES="${BLOCK_STATUS_CODES:-403}"
WAF_NAME="${WAF_NAME:-Nerve-WAF}"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_DIR="${REPORT_ROOT}/www-${STAMP}"

mkdir -p "$OUT_DIR"
echo ">> GoTestWAF target=${TARGET_URL}"
echo ">> report dir=${OUT_DIR}"
echo ">> blockStatusCodes=${BLOCK_STATUS_CODES} wafName=${WAF_NAME}"

docker pull "$IMAGE"
docker run --rm \
  -v "${OUT_DIR}:/app/reports" \
  "$IMAGE" \
  --url="$TARGET_URL" \
  --reportPath=/app/reports \
  --reportName="www-${STAMP}" \
  --noEmailReport \
  --includePayloads="${INCLUDE_PAYLOADS}" \
  --blockStatusCodes="${BLOCK_STATUS_CODES}" \
  --passStatusCodes=200,404 \
  --skipWAFIdentification \
  --wafName="${WAF_NAME}" \
  "$@"

echo ">> done: ${OUT_DIR}"
ls -la "$OUT_DIR" || true
