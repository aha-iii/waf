#!/usr/bin/env bash
# 确保 npm 依赖与当前 OS 匹配（修复 Windows node_modules 在 WSL 下运行失败）
ensure_npm_deps() {
  local app_dir="$1"
  cd "$app_dir"

  local os
  os="$(uname -s)"
  local need_install=0

  if [[ ! -d node_modules ]]; then
    need_install=1
  elif [[ ! -d node_modules/@nerve/ui ]]; then
    echo "[npm] missing @nerve/ui link，重装依赖"
    need_install=1
  elif [[ "$os" == "Linux" ]]; then
    if ! compgen -G "node_modules/@rollup/rollup-linux-*" >/dev/null; then
      echo "[npm] node_modules 为 Windows/其他平台安装，WSL/Linux 下需重装"
      rm -rf node_modules package-lock.json
      need_install=1
    fi
  elif [[ "$os" == MINGW* ]] || [[ "$os" == MSYS* ]] || [[ "$os" == CYGWIN* ]]; then
    if ! compgen -G "node_modules/@rollup/rollup-win32-*" >/dev/null; then
      echo "[npm] node_modules 为 Linux 安装，Windows 下需重装"
      rm -rf node_modules package-lock.json
      need_install=1
    fi
  fi

  if [[ "$need_install" -eq 1 ]]; then
    echo "[npm] npm install → $(pwd)"
    npm install
  fi
}
