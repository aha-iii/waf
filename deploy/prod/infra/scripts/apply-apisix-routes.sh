#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 将 apisix/routes.json 导入 APISIX Admin API（可在 infra 目录或远端 DEPLOY_INFRA_DIR 执行）
#
#   cd deploy/prod/infra && bash scripts/apply-apisix-routes.sh
#   APISIX_ADMIN=http://127.0.0.1:9180 bash scripts/apply-apisix-routes.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INFRA_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROUTES_FILE="${ROUTES_FILE:-$INFRA_DIR/apisix/routes.json}"

if [[ -f "$INFRA_DIR/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source <(sed 's/\r$//' "$INFRA_DIR/.env")
  set +a
fi

ADMIN="${APISIX_ADMIN:-http://127.0.0.1:9180}"
KEY="${APISIX_ADMIN_KEY:-edd1c9f034335f136f87ad84b625c8f1}"
WAF_DOMAIN="${WAF_DOMAIN:-waf.ahatec.com.cn}"
ALERT_DOMAIN="${ALERT_DOMAIN:-alert.ahatec.com.cn}"
WWW_DOMAIN="${WWW_DOMAIN:-www.ahatec.com.cn}"
ROOT_DOMAIN="${ROOT_DOMAIN:-ahatec.com.cn}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "missing command: $1" >&2; exit 1; }
}
need_cmd curl
need_cmd python3

if [[ ! -f "$ROUTES_FILE" ]]; then
  echo "missing routes file: $ROUTES_FILE" >&2
  exit 1
fi

echo ">> apply APISIX routes → ${ADMIN} (domains: ${WAF_DOMAIN} / ${ALERT_DOMAIN} / ${WWW_DOMAIN} / ${ROOT_DOMAIN})"

# 展开 ${ALERT_DOMAIN} 等占位，再逐条 PUT
EXPANDED="$(
  WAF_DOMAIN="$WAF_DOMAIN" ALERT_DOMAIN="$ALERT_DOMAIN" WWW_DOMAIN="$WWW_DOMAIN" ROOT_DOMAIN="$ROOT_DOMAIN" \
  python3 - "$ROUTES_FILE" <<'PY'
import json, os, sys
path = sys.argv[1]
raw = open(path, encoding="utf-8").read()
for k in ("WAF_DOMAIN", "ALERT_DOMAIN", "WWW_DOMAIN", "ROOT_DOMAIN"):
    raw = raw.replace("${%s}" % k, os.environ.get(k, ""))
data = json.loads(raw)
print(json.dumps(data["routes"], ensure_ascii=False))
PY
)"

python3 -c 'import json,sys; json.load(sys.stdin)' <<<"$EXPANDED" >/dev/null

COUNT=0
while IFS= read -r route; do
  [[ -z "$route" || "$route" == "null" ]] && continue
  id="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$route")"
  echo "  → route $id"
  code="$(curl -sS -o /tmp/apisix-route-resp.json -w '%{http_code}' \
    -X PUT "${ADMIN}/apisix/admin/routes/${id}" \
    -H "X-API-KEY: ${KEY}" \
    -H "Content-Type: application/json" \
    -d "$route" || true)"
  if [[ "$code" != "200" && "$code" != "201" ]]; then
    echo "ERROR: PUT route $id → HTTP $code" >&2
    cat /tmp/apisix-route-resp.json >&2 || true
    exit 1
  fi
  COUNT=$((COUNT + 1))
done < <(python3 -c 'import json,sys; [print(json.dumps(r, ensure_ascii=False)) for r in json.load(sys.stdin)]' <<<"$EXPANDED")

echo ">> applied ${COUNT} routes"
