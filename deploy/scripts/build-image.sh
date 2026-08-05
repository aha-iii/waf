#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

WAF_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REPO_ROOT="$(cd "$WAF_ROOT/.." && pwd)"
TAG="${TAG:-1.0.0}"
IMAGE="${IMAGE:-nerve/nerve-waf-api:${TAG}}"
OUT="${OUT:-$WAF_ROOT/dist/nerve-waf-api-${TAG}.tar}"
USE_PREBUILT="${USE_PREBUILT:-false}"
DOCKER_PLATFORM="${DOCKER_PLATFORM:-linux/amd64}"
VITE_CONSOLE_TIME_ZONE="${VITE_CONSOLE_TIME_ZONE:-Asia/Shanghai}"
BUILD_ARGS=(--build-arg "VITE_CONSOLE_TIME_ZONE=${VITE_CONSOLE_TIME_ZONE}")

mkdir -p "$(dirname "$OUT")"

if [[ "$USE_PREBUILT" == "true" ]]; then
  echo "build: prebuilt binaries in ${WAF_ROOT}/target/release"
  if [[ ! -x "$WAF_ROOT/target/release/nerve-waf-api" ]]; then
    echo "missing $WAF_ROOT/target/release/nerve-waf-api — run: cargo build --release -p nerve-waf-api -p nerve-waf-edge"
    exit 1
  fi
  if [[ ! -x "$WAF_ROOT/target/release/nerve-waf-edge" ]]; then
    echo "missing $WAF_ROOT/target/release/nerve-waf-edge — run: cargo build --release -p nerve-waf-api -p nerve-waf-edge"
    exit 1
  fi
  ARTIFACT_DIR="$WAF_ROOT/deploy/docker/artifacts"
  mkdir -p "$ARTIFACT_DIR"
  cp -f "$WAF_ROOT/target/release/nerve-waf-api" "$ARTIFACT_DIR/nerve-waf-api"
  cp -f "$WAF_ROOT/target/release/nerve-waf-edge" "$ARTIFACT_DIR/nerve-waf-edge"
  docker build --platform "$DOCKER_PLATFORM" \
    "${BUILD_ARGS[@]}" \
    -f "$WAF_ROOT/deploy/docker/Dockerfile.prebuilt" \
    -t "$IMAGE" "$REPO_ROOT"
else
  echo "build: compile in container (context=${REPO_ROOT})"
  docker build --platform "$DOCKER_PLATFORM" \
    "${BUILD_ARGS[@]}" \
    -f "$WAF_ROOT/deploy/docker/Dockerfile" \
    -t "$IMAGE" "$REPO_ROOT"
fi

docker save -o "$OUT" "$IMAGE"
echo "saved: $OUT"
echo "load: docker load -i $(basename "$OUT")"
