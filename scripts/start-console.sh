#!/usr/bin/env bash
# Nerve WAF Console — :5174
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONSOLE="${ROOT}/apps/console-react"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck disable=SC1091
source "${SCRIPT_DIR}/npm-deps.sh"
cd "$CONSOLE"

need_node=20
node_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
if [[ "${node_major}" -lt "${need_node}" ]]; then
  echo "[start-console] ERROR: Node.js >= ${need_node} required (current: $(node -v 2>/dev/null || echo missing))"
  echo "  WSL: curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs"
  echo "  或: nvm install 20 && nvm use 20"
  exit 1
fi

echo "============================================"
echo " Nerve WAF Console — http://localhost:5174"
echo " Node $(node -v) | 代理 /api → WAF API :8781（topology→Control :8780）"
echo "============================================"

if command -v ss >/dev/null 2>&1; then
  if ss -tln | grep -q ':5174 '; then
    echo ""
    echo "[start-console] WARN: 端口 5174 已被占用（可能是旧 Console，菜单不会更新）"
    ss -tlnp 2>/dev/null | grep ':5174 ' || true
    echo "  释放端口: fuser -k 5174/tcp"
    echo "  或手动结束旧 vite 进程后再启动"
    echo ""
    if [[ "${CONSOLE_FORCE:-}" == "1" ]]; then
      echo "[start-console] CONSOLE_FORCE=1 → 结束占用 5174 的进程"
      fuser -k 5174/tcp 2>/dev/null || true
      sleep 1
    else
      echo "  强制重启: CONSOLE_FORCE=1 bash ./scripts/start-console.sh"
      exit 1
    fi
  fi
fi

ensure_npm_deps "${CONSOLE}"

# WSL + /mnt/d：Vite 文件监视易触发 EMFILE，提升 ulimit / inotify
if [[ "$(pwd)" == /mnt/* ]] || [[ -n "${WSL_DISTRO_NAME:-}" ]]; then
  ulimit -n 65536 2>/dev/null || true
  for key in fs.inotify.max_user_watches fs.inotify.max_user_instances; do
    cur="$(sysctl -n "${key}" 2>/dev/null || echo 0)"
    if [[ "${cur}" =~ ^[0-9]+$ ]] && [[ "${cur}" -lt 524288 ]]; then
      sysctl -w "${key}=524288" 2>/dev/null || sudo sysctl -w "${key}=524288" 2>/dev/null || true
    fi
  done
  echo "[start-console] WSL: ulimit -n=$(ulimit -n 2>/dev/null || echo n/a), inotify watches=$(sysctl -n fs.inotify.max_user_watches 2>/dev/null || echo n/a)"
fi

exec npm run dev
