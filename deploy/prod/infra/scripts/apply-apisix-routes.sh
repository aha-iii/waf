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
GIT_DOMAIN="${GIT_DOMAIN:-git.ahatec.com.cn}"
DDOS_DOMAIN="${DDOS_DOMAIN:-ddos.ahatec.com.cn}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "missing command: $1" >&2; exit 1; }
}
need_cmd curl
need_cmd python3

if [[ ! -f "$ROUTES_FILE" ]]; then
  echo "missing routes file: $ROUTES_FILE" >&2
  exit 1
fi

echo ">> apply APISIX routes → ${ADMIN} (domains: ${WAF_DOMAIN} / ${ALERT_DOMAIN} / ${WWW_DOMAIN} / ${ROOT_DOMAIN} / ${GIT_DOMAIN} / ${DDOS_DOMAIN})"

# 展开 ${ALERT_DOMAIN} 等占位，再逐条 PUT
EXPANDED="$(
  WAF_DOMAIN="$WAF_DOMAIN" ALERT_DOMAIN="$ALERT_DOMAIN" WWW_DOMAIN="$WWW_DOMAIN" ROOT_DOMAIN="$ROOT_DOMAIN" GIT_DOMAIN="$GIT_DOMAIN" DDOS_DOMAIN="$DDOS_DOMAIN" \
  python3 - "$ROUTES_FILE" <<'PY'
import json, os, sys
path = sys.argv[1]
raw = open(path, encoding="utf-8").read()
for k in ("WAF_DOMAIN", "ALERT_DOMAIN", "WWW_DOMAIN", "ROOT_DOMAIN", "GIT_DOMAIN", "DDOS_DOMAIN"):
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

# 删除本清单之外的旧路由（如过渡态 waf-edge→ddos，否则同 priority 会环回）
KEEP_IDS="$(
  python3 -c 'import json,sys; print(" ".join(r["id"] for r in json.load(sys.stdin)))' <<<"$EXPANDED"
)"
EXISTING="$(curl -fsS -H "X-API-KEY: ${KEY}" "${ADMIN}/apisix/admin/routes" || true)"
if [[ -n "$EXISTING" ]]; then
  while IFS= read -r oid; do
    [[ -z "$oid" ]] && continue
    keep=false
    for kid in $KEEP_IDS; do
      if [[ "$oid" == "$kid" ]]; then keep=true; break; fi
    done
    if [[ "$keep" != "true" ]]; then
      echo "  ✂ delete stale route $oid"
      curl -fsS -o /dev/null -X DELETE -H "X-API-KEY: ${KEY}" \
        "${ADMIN}/apisix/admin/routes/${oid}" || true
    fi
  done < <(python3 -c 'import json,sys
d=json.load(sys.stdin)
for it in d.get("list") or []:
    v=it.get("value") or it
    print(v.get("id") or "")
' <<<"$EXISTING")
fi

GLOBAL_FILE="${GLOBAL_RULES_FILE:-$INFRA_DIR/apisix/global_rules.json}"
if [[ -f "$GLOBAL_FILE" ]]; then
  echo ">> apply APISIX global_rules (Server header / baseline rewrite)"
  EXPAND_G="$(
    WAF_DOMAIN="$WAF_DOMAIN" python3 - "$GLOBAL_FILE" <<'PY'
import json, os, sys
raw = open(sys.argv[1], encoding="utf-8").read()
raw = raw.replace("${WAF_DOMAIN}", os.environ.get("WAF_DOMAIN", ""))
data = json.loads(raw)
print(json.dumps(data.get("global_rules") or data, ensure_ascii=False))
PY
  )"
  while IFS= read -r rule; do
    [[ -z "$rule" || "$rule" == "null" ]] && continue
    gid="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$rule")"
    echo "  → global_rule $gid"
    code="$(curl -sS -o /tmp/apisix-global-resp.json -w '%{http_code}' \
      -X PUT "${ADMIN}/apisix/admin/global_rules/${gid}" \
      -H "X-API-KEY: ${KEY}" \
      -H "Content-Type: application/json" \
      -d "$rule" || true)"
    if [[ "$code" != "200" && "$code" != "201" ]]; then
      echo "ERROR: PUT global_rule $gid → HTTP $code" >&2
      cat /tmp/apisix-global-resp.json >&2 || true
      exit 1
    fi
  done < <(python3 -c 'import json,sys; d=json.load(sys.stdin); rules=d if isinstance(d,list) else d.get("global_rules",d); [print(json.dumps(r, ensure_ascii=False)) for r in rules]' <<<"$EXPAND_G")
fi

echo ">> APISIX apply done"
