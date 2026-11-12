#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 检测 monorepo（Nerve-ui）并选择 Compose 组合。
#
#   cd deploy
#   bash scripts/up.sh
#   bash scripts/up.sh --down

DEPLOY="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$DEPLOY/.." && pwd)"
cd "$DEPLOY"

# Console UI 源码定位：显式 NERVE_UI_DIR 优先，其次仓库同级 / 旧 monorepo 布局。
# 找不到就走 standalone overlay，不阻断启动（无 UI 也能跑 API + Agent）。
UI_DIR=""
for cand in "${NERVE_UI_DIR:-}" "$REPO_ROOT/../Nerve-ui" "$REPO_ROOT/../../Nerve-ui"; do
  if [ -n "$cand" ] && [ -f "$cand/package.json" ]; then
    UI_DIR="$cand"
    break
  fi
done

COMPOSE=(docker compose)
FILES=(-f docker-compose.yml)
ACTION=(up -d --build)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --down)
      ACTION=(down --remove-orphans)
      shift
      ;;
    --no-build)
      ACTION=(up -d)
      shift
      ;;
    -h | --help)
      echo "usage: $(basename "$0") [--down] [--no-build]"
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

if [ -n "$UI_DIR" ]; then
  echo "[up] Console UI 源码: $UI_DIR → 完整栈（含 Console 容器）"
else
  echo "[up] 未发现 Nerve-ui → standalone：仅 Controller + Agent（无 Console）" >&2
  echo "     需要 Web Console: NERVE_UI_DIR=/path/to/Nerve-ui bash scripts/up.sh" >&2
  FILES+=(-f docker-compose.standalone.yml)
fi

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "[up] created .env from .env.example"
fi

# 端口提示以 .env 为准（compose 也从 .env 取值），避免改了 .env 但提示仍是默认端口
envval() {
  local v
  v="$(sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*//p" .env 2>/dev/null | tail -1 | sed 's/[[:space:]]*$//')"
  printf '%s' "${v:-$2}"
}

echo "[up] ${COMPOSE[*]} ${FILES[*]} ${ACTION[*]}"
"${COMPOSE[@]}" "${FILES[@]}" "${ACTION[@]}"

if [[ "${ACTION[0]}" == "up" ]]; then
  echo ""
  echo "Controller : http://127.0.0.1:$(envval PORT 9000)/api/health"
  if [[ " ${FILES[*]} " == *" standalone.yml "* ]]; then
    echo "Console    : (未启动 — standalone 模式；需 UI 请设 NERVE_UI_DIR 后重跑)"
  else
    echo "Console    : http://127.0.0.1:$(envval CONSOLE_PORT 9090)/"
  fi
  echo "Agent      : http://127.0.0.1:$(envval WAF_AGENT_PORT 9080)/health"
  echo "Docs       : deploy/START.md"
fi
