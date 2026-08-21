#!/usr/bin/env bash
# console-react 构建（start-api / start-nerve-waf-expert / pack-dist-runtime 共用）

console_build_init() {
  local root="${1:?waf root}"
  CONSOLE_BUILD_ROOT="$root"
  CONSOLE_DIR="$root/apps/console-react"
  CONSOLE_DIST="$CONSOLE_DIR/dist"
  CONSOLE_MONO_ROOT="$(cd "$root/../.." && pwd)"
}

ensure_nerve_ui_dist() {
  local ui_root="${CONSOLE_MONO_ROOT:-}/Nerve-ui"
  if [[ ! -d "$ui_root" ]]; then
    echo "[nerve-ui] missing $ui_root" >&2
    return 1
  fi
  if [[ -f "$ui_root/dist/index.d.ts" ]] && grep -q 'ToastProvider' "$ui_root/dist/index.d.ts" 2>/dev/null; then
    if ! find "$ui_root/src" -type f \( -name '*.ts' -o -name '*.tsx' \) -newer "$ui_root/dist/index.d.ts" -print -quit 2>/dev/null | grep -q .; then
      return 0
    fi
    echo "[nerve-ui] src 较 dist 有更新，重新构建"
  fi
  echo "[nerve-ui] build @nerve/ui → dist/"
  _console_prepare_node_path
  if command -v npm >/dev/null 2>&1; then
    (cd "$ui_root" && {
      if [[ -d node_modules ]] && [[ ! -w node_modules ]]; then
        echo "[nerve-ui] node_modules 不可写，删除后重装"
        rm -rf node_modules
      fi
      if ! compgen -G "node_modules/@rollup/rollup-linux-*" >/dev/null 2>&1; then
        npm install
      else
        npm ci 2>/dev/null || npm install
      fi
      npm run build
    })
    return 0
  fi
  if command -v docker >/dev/null 2>&1; then
    docker run --rm \
      -v "${CONSOLE_MONO_ROOT}:/src" \
      -w /src/Nerve-ui \
      node:20-bookworm \
      bash -lc 'npm ci && npm run build'
    return 0
  fi
  echo "[nerve-ui] need npm or docker to build Nerve-ui" >&2
  return 1
}

_console_prepare_node_path() {
  if [[ -s "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]]; then
    # shellcheck disable=SC1090
    . "${NVM_DIR:-$HOME/.nvm}/nvm.sh"
    nvm use 20 >/dev/null 2>&1 || nvm use --lts >/dev/null 2>&1 || true
  fi
  local cleaned="" p
  IFS=':' read -r -a _path_parts <<< "${PATH}"
  for p in "${_path_parts[@]}"; do
    case "$p" in
      /mnt/c/*|/mnt/d/*|*nvm4w*) continue ;;
    esac
    if [[ -n "$cleaned" ]]; then
      cleaned="${cleaned}:${p}"
    else
      cleaned="${p}"
    fi
  done
  PATH="$cleaned"
  export PATH
}

build_console_react() {
  [[ -n "${CONSOLE_DIR:-}" ]] || { echo "[console] console_build_init 未调用" >&2; return 1; }

  ensure_nerve_ui_dist || return 1
  _console_prepare_node_path

  local need_node=18 major=0
  if command -v node >/dev/null 2>&1; then
    major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
  fi

  if [[ "$major" -ge "$need_node" ]]; then
    echo "[console] node $(node -v) → npm run build"
    local lib_dir script_dir
    lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    script_dir="$(cd "$lib_dir/.." && pwd)"
    # shellcheck source=../npm-deps.sh
    source "$script_dir/npm-deps.sh"
    ensure_npm_deps "$CONSOLE_DIR"
    if (cd "$CONSOLE_DIR" && npm run build); then
      return 0
    fi
    echo "[console] 本地 npm 构建失败，尝试 docker 构建"
  fi

  if command -v docker >/dev/null 2>&1; then
    local lib_dir script_dir
    lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    script_dir="$(cd "$lib_dir/.." && pwd)"
    echo "[console] 本地 npm 构建失败，尝试 docker 构建"
    bash "$script_dir/docker-build-console.sh"
    return $?
  fi

  echo "[console] 需要 Node >= $need_node（当前 ${major:-none}）或 Docker" >&2
  return 1
}

ensure_console_dist() {
  if [[ "${NERVE_WAF_CONSOLE_SKIP_BUILD:-0}" == "1" ]]; then
    echo "[console] 跳过构建 (NERVE_WAF_CONSOLE_SKIP_BUILD=1)"
    return 0
  fi
  if [[ "${NERVE_WAF_CONSOLE_FORCE_BUILD:-0}" == "1" ]]; then
    echo "[console] 强制重建 (NERVE_WAF_CONSOLE_FORCE_BUILD=1)"
    build_console_react
    [[ -f "$CONSOLE_DIST/index.html" ]] || return 1
    return 0
  fi
  if [[ -f "$CONSOLE_DIST/index.html" ]]; then
    local marker="$CONSOLE_DIR/src/auth.ts"
    local asset
    asset="$(find "$CONSOLE_DIST/assets" -maxdepth 1 -name 'index-*.js' -print -quit 2>/dev/null || true)"
    if [[ -f "$marker" && -n "$asset" && "$marker" -nt "$asset" ]]; then
      echo "[console] 源码较新 ($marker)，重新构建 dist"
      build_console_react
      [[ -f "$CONSOLE_DIST/index.html" ]] || return 1
      return 0
    fi
    echo "[console] dist 已就绪: $CONSOLE_DIST"
    return 0
  fi
  echo "[console] 构建 console-react → $CONSOLE_DIST"
  build_console_react
  [[ -f "$CONSOLE_DIST/index.html" ]] || {
    echo "[console] 构建失败: 缺少 $CONSOLE_DIST/index.html" >&2
    return 1
  }
}
