#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 对 WAF Host（site-waf-console）跑官方 GoTestWAF。
#
# 注意：正式环境实测 https://waf.ahatec.com.cn/ 对典型 XSS/SQLi 探针仍返回 200
#（回源 console SPA，未像官网那样 403），GoTestWAF 预检会失败。
# 评测拦截率请优先用 scripts/gotestwaf-www.sh。
#
# 若仍要打 waf Host（接受预检可能失败，或强制跳过）：
#   SKIP_WAF_BLOCK_CHECK=1 bash scripts/gotestwaf-waf-host.sh
#
#   bash scripts/gotestwaf-waf-host.sh

TARGET_URL="${TARGET_URL:-https://waf.ahatec.com.cn/}"
IMAGE="${GOTESTWAF_IMAGE:-wallarm/gotestwaf:latest}"
REPORT_ROOT="${REPORT_ROOT:-$(pwd)/gotestwaf-reports}"
INCLUDE_PAYLOADS="${INCLUDE_PAYLOADS:-true}"
BLOCK_STATUS_CODES="${BLOCK_STATUS_CODES:-403}"
WAF_NAME="${WAF_NAME:-Nerve-WAF}"
SKIP_WAF_BLOCK_CHECK="${SKIP_WAF_BLOCK_CHECK:-0}"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_DIR="${REPORT_ROOT}/waf-host-${STAMP}"

mkdir -p "$OUT_DIR"
echo ">> GoTestWAF target=${TARGET_URL}"
echo ">> report dir=${OUT_DIR}"
echo ">> blockStatusCodes=${BLOCK_STATUS_CODES} wafName=${WAF_NAME}"

# 预检：典型探针是否已是 403
PROBE_CODE="$(curl -sS -o /dev/null -w '%{http_code}' -A 'GoTestWAF-Probe' --max-time 15 \
  "${TARGET_URL}?q=%3Cscript%3Ealert%281%29%3C%2Fscript%3E" || echo 000)"
echo ">> preflight XSS probe HTTP ${PROBE_CODE}"
if [[ "$PROBE_CODE" != "403" && "$SKIP_WAF_BLOCK_CHECK" != "1" ]]; then
  echo "ERROR: ${TARGET_URL} 未对探针返回 403（实际 ${PROBE_CODE}）。" >&2
  echo "  官网评测请改用: bash scripts/gotestwaf-www.sh" >&2
  echo "  若坚持测本 Host: SKIP_WAF_BLOCK_CHECK=1 bash scripts/gotestwaf-waf-host.sh" >&2
  echo "  （跳过预检后 bypass 会偏高，因当前 Host 可能根本未拦）" >&2
  exit 2
fi

EXTRA=()
if [[ "$SKIP_WAF_BLOCK_CHECK" == "1" ]]; then
  EXTRA+=(--skipWAFBlockCheck)
  echo ">> WARNING: --skipWAFBlockCheck enabled"
fi

docker pull "$IMAGE"
docker run --rm \
  -v "${OUT_DIR}:/app/reports" \
  "$IMAGE" \
  --url="$TARGET_URL" \
  --reportPath=/app/reports \
  --reportName="waf-host-${STAMP}" \
  --noEmailReport \
  --includePayloads="${INCLUDE_PAYLOADS}" \
  --blockStatusCodes="${BLOCK_STATUS_CODES}" \
  --passStatusCodes=200,404 \
  --skipWAFIdentification \
  --wafName="${WAF_NAME}" \
  "${EXTRA[@]}" \
  "$@"

echo ">> done: ${OUT_DIR}"
ls -la "$OUT_DIR" || true
