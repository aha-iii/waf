#!/usr/bin/env bash
if [ -z "${BASH_VERSION:-}" ]; then
  exec /usr/bin/env bash "$0" "$@"
fi

set -euo pipefail

# Apply Nerve WAF PostgreSQL migrations (control-plane store).
# Run outside application processes — see repo DDL separation policy.
#
# Usage:
#   DATABASE_URL=postgres://user:pass@127.0.0.1:5432/nerve bash scripts/apply-migrations.sh
#
# Requires: psql (PostgreSQL client). Each migration file runs at most once
# (tracked in nerve_waf_schema_migrations).

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MIG_DIR="$ROOT/crates/controlplane/nerve-waf-store/migrations"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1" >&2
    exit 1
  }
}

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "set DATABASE_URL=postgres://user:pass@host:5432/nerve" >&2
  exit 1
fi

if [[ "${DATABASE_URL}" != postgres:* ]]; then
  echo "only postgres DATABASE_URL is supported (got: ${DATABASE_URL%%:*}:...)" >&2
  exit 1
fi

need_cmd psql

echo "applying WAF migrations -> ${DATABASE_URL%%@*}@***"

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
CREATE TABLE IF NOT EXISTS nerve_waf_schema_migrations (
    filename TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
SQL

shopt -s nullglob
files=("$MIG_DIR"/*.sql)
if ((${#files[@]} == 0)); then
  echo "no migration files under $MIG_DIR" >&2
  exit 1
fi

IFS=$'\n' sorted=($(printf '%s\n' "${files[@]}" | sort))
unset IFS

for f in "${sorted[@]}"; do
  base="$(basename "$f")"
  applied="$(psql "$DATABASE_URL" -tAc "SELECT 1 FROM nerve_waf_schema_migrations WHERE filename = '$base' LIMIT 1" 2>/dev/null | tr -d '[:space:]' || true)"
  if [[ "$applied" == "1" ]]; then
    echo "  skip $base (already applied)"
    continue
  fi
  echo "  -> $base"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "INSERT INTO nerve_waf_schema_migrations (filename) VALUES ('$base')"
done

echo "WAF postgres migrations applied (${#sorted[@]} files checked)"
