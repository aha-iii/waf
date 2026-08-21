#!/usr/bin/env bash
# 将 Engram Gateway 编译产物 + dev artifact 打入 dist/runtime/engram/（mars-test 发布用）

pack_engram_runtime() {
  local waf_root="${1:?waf root}"
  local mono_root="${2:?mono root}"
  local out="${3:?output dir}"

  local llm_root="$mono_root/Nerve-llm"
  local artifact_src="$llm_root/training/data/engram-linear-v1.json"
  local gateway_dist="$llm_root/dist/gateway/src/server.js"

  if [[ ! -d "$llm_root" ]]; then
    echo "pack-engram: skip — missing $llm_root" >&2
    return 0
  fi

  if [[ ! -f "$gateway_dist" ]]; then
    echo ">> pack engram: npm run build (Nerve-llm)"
    if command -v npm >/dev/null 2>&1; then
      (cd "$llm_root" && npm run build)
    elif command -v docker >/dev/null 2>&1; then
      docker run --rm \
        -v "$mono_root:/src" \
        -w /src/Nerve-llm \
        node:20-bookworm \
        bash -lc 'npm ci && npm run build'
      gateway_dist="$llm_root/dist/gateway/src/server.js"
    else
      echo "pack-engram: skip — need npm or docker to build Nerve-llm gateway" >&2
      return 0
    fi
  fi

  if [[ ! -f "$gateway_dist" ]]; then
    echo "pack-engram: skip — missing $gateway_dist" >&2
    return 0
  fi

  rm -rf "$out/engram"
  mkdir -p "$out/engram/artifact" "$out/engram/gateway-dist/src"
  cp -f "$gateway_dist" "$out/engram/gateway-dist/src/server.js"
  if [[ -f "$artifact_src" ]]; then
    cp -f "$artifact_src" "$out/engram/artifact/engram-linear-v1.json"
  else
    echo ">> pack engram: bootstrap dev artifact"
    cat > "$out/engram/artifact/engram-linear-v1.json" <<'EOF'
{
  "schema": "engram-linear-v1",
  "threshold": 0.5,
  "patterns": [],
  "category_weights": {},
  "stats": { "records": 0, "patterns": 0, "categories": 0, "note": "mars-test bootstrap" }
}
EOF
  fi
  echo ">> pack engram: $out/engram"
}
