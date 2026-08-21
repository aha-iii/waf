#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# 对标 Nginx worker_processes：在同一端口起 N 个 Edge 进程，依赖 SO_REUSEPORT 由内核分流连接。
# 环境变量：
#   NERVE_EDGE_WORKERS   auto | 1..64（默认 auto = nproc）
#   NERVE_EDGE_REUSEPORT 1|0（多 worker 时必须 1，默认 1）
#   WAF_EDGE_PORT / WAF_EDGE_API / NERVE_WAF_PLUGINS_DIR / NERVE_WAF_EDGE_BIN

EDGE_BIN="${NERVE_WAF_EDGE_BIN:-nerve-waf-edge}"
PORT="${WAF_EDGE_PORT:-8080}"
API="${WAF_EDGE_API:-http://127.0.0.1:8781}"
PLUGINS="${NERVE_WAF_PLUGINS_DIR:-}"

workers_raw="${NERVE_EDGE_WORKERS:-auto}"
if [[ "$workers_raw" == "auto" ]]; then
  if command -v nproc >/dev/null 2>&1; then
    workers="$(nproc)"
  else
    workers=4
  fi
else
  workers="$workers_raw"
fi

if ! [[ "$workers" =~ ^[0-9]+$ ]]; then
  echo "run-edge-workers: invalid NERVE_EDGE_WORKERS=$workers_raw" >&2
  exit 1
fi

workers=$((workers))
if (( workers < 1 )); then
  workers=1
fi
if (( workers > 64 )); then
  workers=64
fi

reuse_on=1
case "${NERVE_EDGE_REUSEPORT:-1}" in
  0|false|no|off) reuse_on=0 ;;
esac

if (( workers > 1 && reuse_on == 0 )); then
  echo "run-edge-workers: NERVE_EDGE_WORKERS=$workers requires NERVE_EDGE_REUSEPORT=1; forcing 1 worker" >&2
  workers=1
fi

if (( reuse_on )); then
  export NERVE_EDGE_REUSEPORT=1
else
  unset NERVE_EDGE_REUSEPORT
fi

echo "run-edge-workers: $workers worker(s) on :$PORT api=$API reuseport=${reuse_on}"

edge_args=(--port "$PORT" --api "$API")
if [[ -n "$PLUGINS" ]]; then
  edge_args+=(--plugins-dir "$PLUGINS")
fi

pids=()
cleanup() {
  local pid
  for pid in "${pids[@]:-}"; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup TERM INT

for ((i = 0; i < workers; i++)); do
  "$EDGE_BIN" "${edge_args[@]}" &
  pids+=("$!")
done

status=0
for pid in "${pids[@]}"; do
  if ! wait "$pid"; then
    status=1
  fi
done
exit "$status"
