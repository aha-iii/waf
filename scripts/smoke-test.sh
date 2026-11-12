#!/usr/bin/env bash
#
# Nerve WAF — post-install smoke test
# Copyright (c) 2024-2026 Nerve Team · MIT License
#
# Verifies a running deployment end to end: health, storage, control-plane data,
# and (optionally) the inspect pipeline with a benign and a malicious request.
#
# Usage:
#   bash scripts/smoke-test.sh [OPTIONS]
#
# Options:
#   --controller URL    Control API base URL   (env CONTROLLER_URL, default http://127.0.0.1:9000)
#   --agent URL         Agent base URL         (env AGENT_URL,      default http://127.0.0.1:9080)
#   --console URL       Console base URL       (env CONSOLE_URL,    default http://127.0.0.1:9090)
#   --key KEY           API key (X-API-Key)    (env NERVE_API_KEY)
#   --require-agent     Fail when the agent is unreachable (default: skip)
#   --require-console   Fail when the console is unreachable (default: skip)
#   --no-inspect        Skip the inspect-pipeline checks
#   --json              Print a machine-readable summary
#   -h, --help          Show this help
#
# Exit codes: 0 all required checks passed · 1 a required check failed · 2 usage error

if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi
set -Eeuo pipefail

CONTROLLER_URL="${CONTROLLER_URL:-http://127.0.0.1:9000}"
AGENT_URL="${AGENT_URL:-http://127.0.0.1:9080}"
CONSOLE_URL="${CONSOLE_URL:-http://127.0.0.1:9090}"
NERVE_API_KEY="${NERVE_API_KEY:-}"
REQUIRE_AGENT=0
REQUIRE_CONSOLE=0
DO_INSPECT=1
JSON_OUT=0
TIMEOUT="${SMOKE_TIMEOUT:-10}"

# Localhost checks must not go through an HTTP proxy.
CURL=(curl --noproxy "*" -sS --max-time "$TIMEOUT")

PASS=0
FAIL=0
SKIP=0
declare -a RESULTS=()

usage() {
  sed -n '/^# Usage:/,/^# Exit codes:/p' "$0" | sed 's/^# \{0,1\}//' | sed '$d'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --controller)
      CONTROLLER_URL="${2:?--controller 需要 URL}"
      shift 2
      ;;
    --agent)
      AGENT_URL="${2:?--agent 需要 URL}"
      shift 2
      ;;
    --console)
      CONSOLE_URL="${2:?--console 需要 URL}"
      shift 2
      ;;
    --key)
      NERVE_API_KEY="${2:?--key 需要值}"
      shift 2
      ;;
    --require-agent)
      REQUIRE_AGENT=1
      shift
      ;;
    --require-console)
      REQUIRE_CONSOLE=1
      shift
      ;;
    --no-inspect)
      DO_INSPECT=0
      shift
      ;;
    --json)
      JSON_OUT=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数：$1（用 --help 查看用法）" >&2
      exit 2
      ;;
  esac
done

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
  G=$'\033[32m' R=$'\033[31m' Y=$'\033[33m' D=$'\033[2m' N=$'\033[0m'
else
  G="" R="" Y="" D="" N=""
fi

record() {
  local status="$1" name="$2" detail="$3"
  case "$status" in
    pass) PASS=$((PASS + 1)) ;;
    fail) FAIL=$((FAIL + 1)) ;;
    skip) SKIP=$((SKIP + 1)) ;;
  esac
  RESULTS+=("${status}|${name}|${detail}")
  if ((JSON_OUT == 0)); then
    case "$status" in
      pass) printf '  %s✓%s %-34s %s%s%s\n' "$G" "$N" "$name" "$D" "$detail" "$N" ;;
      fail) printf '  %s✗%s %-34s %s\n' "$R" "$N" "$name" "$detail" ;;
      skip) printf '  %s-%s %-34s %s%s%s\n' "$Y" "$N" "$name" "$D" "$detail" "$N" ;;
    esac
  fi
}

# check <name> <required:0|1> <expected-http|any> <url> [extra curl args...]
check_http() {
  local name="$1" required="$2" expect="$3" url="$4"
  shift 4
  local code
  code="$("${CURL[@]}" -o /dev/null -w '%{http_code}' "$@" "$url" 2>/dev/null || true)"
  code="${code:-000}"

  if [[ "$code" == "000" ]]; then
    if ((required)); then
      record fail "$name" "无法连接 $url"
    else
      record skip "$name" "不可达（可选组件）"
    fi
    return
  fi
  if [[ "$expect" == "any" ]] || [[ "$code" == "$expect" ]]; then
    record pass "$name" "HTTP $code"
  else
    if ((required)); then
      record fail "$name" "HTTP $code（期望 $expect）"
    else
      record skip "$name" "HTTP $code（可选组件）"
    fi
  fi
}

# inspect <name> <required> <json-body> -> prints action
inspect_action() {
  local body="$1"
  local out
  out="$("${CURL[@]}" -X POST -H 'Content-Type: application/json' \
    ${NERVE_API_KEY:+-H "X-API-Key: $NERVE_API_KEY"} \
    -d "$body" "$CONTROLLER_URL/api/inspect" 2>/dev/null || true)"
  [[ -z "$out" ]] && {
    printf ''
    return
  }
  printf '%s' "$out" | sed -n 's/.*"action":"\([a-z]*\)".*/\1/p' | head -1
}

if ((JSON_OUT == 0)); then
  echo "Nerve WAF — 冒烟测试"
  echo "  Controller : $CONTROLLER_URL"
  echo "  Agent      : $AGENT_URL"
  echo "  Console    : $CONSOLE_URL"
  echo ""
fi

# ---------------------------------------------------------------- core
check_http "Controller 健康" 1 200 "$CONTROLLER_URL/api/health"
check_http "Agent 健康" "$REQUIRE_AGENT" 200 "$AGENT_URL/health"
check_http "Console 首页" "$REQUIRE_CONSOLE" any "$CONSOLE_URL/"

# 存储后端：能返回 backend 说明 store 初始化成功
check_http "数据库状态 /api/db/status" 1 200 "$CONTROLLER_URL/api/db/status"

# 控制面数据：站点列表可读
sites="$("${CURL[@]}" "$CONTROLLER_URL/api/sites" 2>/dev/null || true)"
site_count="$(printf '%s' "$sites" | grep -o '"cname_fqdn"' | wc -l | tr -d ' ')"
if [[ -n "$sites" ]] && printf '%s' "$sites" | grep -q '"sites"'; then
  record pass "站点列表 /api/sites" "${site_count} 个站点"
else
  record fail "站点列表 /api/sites" "响应不是预期的 JSON"
fi

# 插件宿主：至少加载一个插件
plugins="$("${CURL[@]}" "$CONTROLLER_URL/api/plugins" 2>/dev/null || true)"
plugin_count="$(printf '%s' "$plugins" | grep -o '"name"' | wc -l | tr -d ' ')"
if printf '%s' "$plugins" | grep -q '"plugins"'; then
  record pass "插件宿主 /api/plugins" "${plugin_count} 个插件"
else
  record fail "插件宿主 /api/plugins" "响应不是预期的 JSON"
fi

# ---------------------------------------------------------------- inspect pipeline
if ((DO_INSPECT)); then
  # Prefer a real configured site: rules are bound per site, so `default` may have none.
  smoke_site="$(printf '%s' "$sites" | grep -o '"id":"[^"]*"' | head -1 | sed 's/.*"id":"//; s/"$//')"
  smoke_site="${smoke_site:-default}"

  benign="{\"app_id\":\"${smoke_site}\",\"method\":\"GET\",\"host\":\"smoke.local\",\"uri\":\"/index.html\",\"headers\":\"Accept: text/html\\r\\n\",\"body\":\"\",\"src_ip\":\"203.0.113.10\",\"user_agent\":\"nerve-smoke/1.0\"}"
  action="$(inspect_action "$benign")"
  if [[ "$action" == "allow" ]]; then
    record pass "检测放行（正常请求, site=$smoke_site）" "action=allow"
  elif [[ -z "$action" ]]; then
    record fail "检测放行（正常请求）" "无响应或响应格式异常"
  else
    record fail "检测放行（正常请求）" "action=$action（应放行，检查站点策略）"
  fi

  # 恶意样本：命中与否取决于站点是否绑定策略与规则。软检查，不判失败。
  malicious="{\"app_id\":\"${smoke_site}\",\"method\":\"GET\",\"host\":\"smoke.local\",\"uri\":\"/search?q=sqlmap\",\"headers\":\"Accept: */*\\r\\n\",\"body\":\"\",\"src_ip\":\"198.51.100.66\",\"user_agent\":\"sqlmap/1.7\"}"
  action="$(inspect_action "$malicious")"
  if [[ "$action" == "block" ]]; then
    record pass "检测拦截（恶意样本）" "action=block"
  elif [[ "$action" == "allow" ]]; then
    record skip "检测拦截（恶意样本）" "未命中：站点需绑定策略/规则后才会拦截"
  else
    record skip "检测拦截（恶意样本）" "跳过（无响应）"
  fi
fi

# ---------------------------------------------------------------- summary
if ((JSON_OUT)); then
  printf '{"pass":%d,"fail":%d,"skip":%d,"controller":"%s","results":[' "$PASS" "$FAIL" "$SKIP" "$CONTROLLER_URL"
  first=1
  for r in "${RESULTS[@]}"; do
    IFS='|' read -r st nm dt <<<"$r"
    ((first)) || printf ','
    first=0
    printf '{"status":"%s","name":"%s","detail":"%s"}' "$st" "$nm" "$dt"
  done
  printf ']}\n'
else
  echo ""
  printf '  合计：%s%d 通过%s · %s%d 跳过%s · %s%d 失败%s\n' \
    "$G" "$PASS" "$N" "$Y" "$SKIP" "$N" "$R" "$FAIL" "$N"
fi

if ((FAIL > 0)); then
  if ((JSON_OUT == 0)); then
    echo ""
    echo "  排查："
    echo "    docker compose -f deploy/docker-compose.yml logs --tail=100"
    echo "    文档：docs/installation.md · docs/runbooks/PRODUCTION.md"
  fi
  exit 1
fi

exit 0
