#!/usr/bin/env bash
# Nerve-website 构建（pack-dist-runtime 共用）

website_build_init() {
  local root="${1:?repo root}"
  WEBSITE_BUILD_ROOT="$root"
  WEBSITE_DIR="$root/Nerve-website"
  WEBSITE_DIST="$WEBSITE_DIR/dist"
}

_website_prepare_node_path() {
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

build_nerve_website() {
  [[ -n "${WEBSITE_DIR:-}" ]] || { echo "[website] website_build_init 未调用" >&2; return 1; }

  _website_prepare_node_path

  local mode="${1:-${WEBSITE_BUILD_MODE:-production}}"
  local npm_script="build"
  if [[ "$mode" == "test" ]]; then
    npm_script="build:test"
  fi
  echo "[website] build mode=$mode → npm run $npm_script"

  # fail-fast: 确保对应 mode 的 .env.{mode} 存在，避免 Vite 静默回退到 .env（默认正式地址）
  local env_file="$WEBSITE_DIR/.env.${mode}"
  if [[ ! -f "$env_file" ]]; then
    echo "[website] ✗ 缺少 $env_file" >&2
    echo "[website]   Vite --mode $mode 找不到 .env.${mode} 会静默回退到 .env（可能是正式地址）" >&2
    echo "[website]   修复: cp $WEBSITE_DIR/.env.${mode}.example $env_file 并填写正确的地址" >&2
    return 1
  fi
  echo "[website] env ok: .env.${mode} 存在"

  local need_node=18 major=0
  if command -v node >/dev/null 2>&1; then
    major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
  fi

  if [[ "$major" -ge "$need_node" ]]; then
    if [[ -d "$WEBSITE_DIR/node_modules" ]] && ! compgen -G "$WEBSITE_DIR/node_modules/@rollup/rollup-linux-*" >/dev/null 2>&1; then
      if command -v docker >/dev/null 2>&1; then
        echo "[website] 跨平台 node_modules，改用 docker node:20 构建"
        docker run --rm \
          -v "$WEBSITE_BUILD_ROOT:/src" \
          -w /src/Nerve-website \
          node:20-bookworm \
          bash -lc "npm ci && npm run $npm_script"
        return 0
      fi
    fi
    echo "[website] node $(node -v) → npm run $npm_script"
    local lib_dir script_dir
    lib_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    script_dir="$(cd "$lib_dir/.." && pwd)"
    # shellcheck source=../npm-deps.sh
    source "$script_dir/npm-deps.sh"
    ensure_npm_deps "$WEBSITE_DIR"
    (cd "$WEBSITE_DIR" && npm run "$npm_script")
    return 0
  fi

  if command -v docker >/dev/null 2>&1; then
    local repo_root
    repo_root="$WEBSITE_BUILD_ROOT"
    echo "[website] local node < $need_node，使用 docker node:20 构建"
    docker run --rm \
      -v "$repo_root:/src" \
      -w /src/Nerve-website \
      node:20-bookworm \
      bash -lc "npm ci && npm run $npm_script"
    return 0
  fi

  echo "[website] 需要 Node >= $need_node（当前 ${major:-none}）或 Docker" >&2
  return 1
}
