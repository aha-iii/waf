#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 将 nginx 格式证书导入 APISIX Admin SSL（SNI）
#
#   CERT=www.ahatec.com.cn.pem KEY=www.ahatec.com.cn.key \
#   SNIS=www.ahatec.com.cn,ahatec.com.cn \
#   bash scripts/apply-apisix-ssl.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ -f "$INFRA_DIR/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source <(sed 's/\r$//' "$INFRA_DIR/.env")
  set +a
fi

ADMIN="${APISIX_ADMIN:-http://127.0.0.1:9180}"
KEY_HDR="${APISIX_ADMIN_KEY:-edd1c9f034335f136f87ad84b625c8f1}"
CERT_FILE="${CERT:-$INFRA_DIR/certs/www.ahatec.com.cn.pem}"
KEY_FILE="${KEY:-$INFRA_DIR/certs/www.ahatec.com.cn.key}"
SNIS_CSV="${SNIS:-www.ahatec.com.cn,ahatec.com.cn}"
SSL_ID="${SSL_ID:-ahatec-www}"

need_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1" >&2; exit 1; }; }
need_cmd curl
need_cmd python3

[[ -f "$CERT_FILE" ]] || { echo "missing cert: $CERT_FILE" >&2; exit 1; }
[[ -f "$KEY_FILE" ]] || { echo "missing key: $KEY_FILE" >&2; exit 1; }

BODY="$(
  CERT_FILE="$CERT_FILE" KEY_FILE="$KEY_FILE" SNIS_CSV="$SNIS_CSV" python3 - <<'PY'
import json, os
cert = open(os.environ["CERT_FILE"], encoding="utf-8").read()
key = open(os.environ["KEY_FILE"], encoding="utf-8").read()
snis = [s.strip() for s in os.environ["SNIS_CSV"].split(",") if s.strip()]
print(json.dumps({"cert": cert, "key": key, "snis": snis}, ensure_ascii=False))
PY
)"

echo ">> PUT ssl id=${SSL_ID} snis=${SNIS_CSV} → ${ADMIN}"
code="$(curl -sS -o /tmp/apisix-ssl-resp.json -w '%{http_code}' \
  -X PUT "${ADMIN}/apisix/admin/ssls/${SSL_ID}" \
  -H "X-API-KEY: ${KEY_HDR}" \
  -H "Content-Type: application/json" \
  -d "$BODY" || true)"
if [[ "$code" != "200" && "$code" != "201" ]]; then
  echo "ERROR: HTTP $code" >&2
  cat /tmp/apisix-ssl-resp.json >&2 || true
  exit 1
fi
echo ">> ssl applied (HTTP $code)"
