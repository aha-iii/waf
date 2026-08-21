# 按需编译 Rust release 二进制（由 start 脚本 source）
#
# 默认：二进制存在则 **不** 调 cargo（避免 WSL /mnt/d 上 mtime 误判导致每次全量编）。
# 改源码后请: bash scripts/start-waf-edge-stack.sh --build
#
# 用法:
#   ensure_rust_release_bins "$ROOT" nerve-waf-api nerve-waf-edge
#   ensure_rust_release_bins "$ROOT" --force nerve-waf-api
#
# 环境变量:
#   ENSURE_RUST_FORCE=1              强制重编
#   ENSURE_RUST_SKIP=1               绝不编译（缺二进制则失败）
#   ENSURE_RUST_REBUILD_ON_CHANGE=1  开启 Cargo.lock/源码 mtime 检测（/mnt/d 易误判）

ensure_rust_release_bins() {
  local root="$1"
  shift
  local force=0
  local skip=0
  local check_stale="${ENSURE_RUST_REBUILD_ON_CHANGE:-0}"
  local -a packages=()

  if [[ "${ENSURE_RUST_FORCE:-0}" == "1" ]]; then
    force=1
  fi
  if [[ "${ENSURE_RUST_SKIP:-0}" == "1" ]]; then
    skip=1
  fi

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --force) force=1; shift ;;
      --skip) skip=1; shift ;;
      *) packages+=("$1"); shift ;;
    esac
  done

  if [[ ${#packages[@]} -eq 0 ]]; then
    echo "[ensure-rust-bins] ERROR: no packages specified" >&2
    return 1
  fi

  local -a missing=()
  local -a stale=()
  local pkg bin crate_dir

  for pkg in "${packages[@]}"; do
    bin="$root/target/release/$pkg"
    crate_dir="$root/crates/$pkg"
    if [[ ! -x "$bin" ]]; then
      missing+=("$pkg")
      continue
    fi
    if [[ "$force" == "1" ]]; then
      stale+=("$pkg")
      continue
    fi
    if [[ "$check_stale" != "1" ]]; then
      continue
    fi
    if [[ -f "$root/Cargo.lock" && "$root/Cargo.lock" -nt "$bin" ]]; then
      stale+=("$pkg")
      continue
    fi
    if [[ -d "$crate_dir" ]]; then
      if find "$crate_dir" \( -name '*.rs' -o -name 'Cargo.toml' \) -newer "$bin" 2>/dev/null | grep -q .; then
        stale+=("$pkg")
      fi
    fi
  done

  if [[ "$skip" == "1" ]]; then
    if [[ ${#missing[@]} -gt 0 ]]; then
      echo "[ensure-rust-bins] ERROR: 缺少二进制: ${missing[*]}（去掉 --skip-build 或先 cargo build）" >&2
      return 1
    fi
    echo "[ensure-rust-bins] 跳过编译（--skip-build）"
    return 0
  fi

  if [[ "$force" != "1" && ${#missing[@]} -eq 0 && ${#stale[@]} -eq 0 ]]; then
    echo "[ensure-rust-bins] 跳过编译（release 二进制已存在；改源码后请加 --build）"
    return 0
  fi

  local -a to_build=("${missing[@]}" "${stale[@]}")
  local -a unique=()
  local u found
  for pkg in "${to_build[@]}"; do
    found=0
    for u in "${unique[@]}"; do
      if [[ "$u" == "$pkg" ]]; then
        found=1
        break
      fi
    done
    [[ "$found" == "0" ]] && unique+=("$pkg")
  done

  local -a cargo_args=(--release)
  for pkg in "${unique[@]}"; do
    cargo_args+=(-p "$pkg")
  done

  echo "[ensure-rust-bins] cargo build ${cargo_args[*]}"
  (cd "$root" && cargo build "${cargo_args[@]}")
}
