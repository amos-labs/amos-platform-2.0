#!/usr/bin/env bash
#
# Migration version-collision guard (Rails-style safety).
#
# Parallel branches that hand-number migrations (YYYYMMDD000001) pick the same
# version and collide on merge: two files claim the same slot, sqlx sees a
# checksum mismatch, and the service crashes on boot.
#
# This guard fails a PR if it adds any migration whose numeric version prefix is
# <= the maximum migration version already on the base branch (main). New
# migrations MUST sort strictly after everything on main.
#
# Prefer `sqlx migrate add <name>`, which stamps a full UTC timestamp and makes
# collisions effectively impossible.
#
# Env:
#   MIGRATIONS_DIR  directory holding the *.sql migrations (default: migrations)
#   BASE_REF        base branch to compare against (default: origin/main)
#
# Requires: a full-history checkout (fetch-depth: 0) so BASE_REF is reachable.
set -euo pipefail

MIGRATIONS_DIR="${MIGRATIONS_DIR:-migrations}"
BASE_REF="${BASE_REF:-origin/main}"

# Given a newline-delimited list of paths on stdin, print the *.sql basenames
# that carry a leading numeric version prefix (digits before the first '_'),
# one per line. Other names are dropped.
migration_basenames() {
  sed -E 's#.*/##' | grep -E '\.sql$' | grep -E '^[0-9]+_'
}

# Numeric version prefix of a migration basename.
version_of() {
  printf '%s\n' "$1" | sed -E 's/^([0-9]+)_.*/\1/'
}

if [ ! -d "$MIGRATIONS_DIR" ]; then
  echo "No migrations directory at '$MIGRATIONS_DIR' — nothing to check."
  exit 0
fi

# Migration file basenames on the base branch (main).
base_files="$(git ls-tree -r --name-only "$BASE_REF" -- "$MIGRATIONS_DIR" 2>/dev/null \
  | migration_basenames | sort -u || true)"
# Versions on base.
base_versions="$(printf '%s\n' "$base_files" | while IFS= read -r f; do
  [ -n "$f" ] && version_of "$f"; done | grep -E '^[0-9]+$' | sort -u || true)"

# Max version on main (empty if main has no migrations yet).
max_base="$(printf '%s\n' "$base_versions" | sort -n | tail -1 || true)"

if [ -z "$max_base" ]; then
  echo "Base branch '$BASE_REF' has no migrations under '$MIGRATIONS_DIR'; no ceiling to enforce."
  exit 0
fi

echo "Latest migration version on $BASE_REF: $max_base"

# Migration file basenames on the PR head (the checked-out working tree).
head_files="$(ls -1 "$MIGRATIONS_DIR"/*.sql 2>/dev/null \
  | migration_basenames | sort -u || true)"

# New migration FILES = present on head but not on base (compared by filename,
# so a new file that reuses an existing version is still caught as new).
new_files="$(comm -23 \
  <(printf '%s\n' "$head_files" | sort -u) \
  <(printf '%s\n' "$base_files" | sort -u) || true)"

new_files="$(printf '%s\n' "$new_files" | grep -E '^[0-9]+_' || true)"

if [ -z "$new_files" ]; then
  echo "No new migrations added in this PR."
  exit 0
fi

fail=0
while IFS= read -r f; do
  [ -z "$f" ] && continue
  v="$(version_of "$f")"
  # A new migration must sort strictly after everything on main. This rejects
  # both a lower/equal version prefix AND a fresh file that reuses an existing
  # version (which would still collide on the checksum).
  if [ "$v" -le "$max_base" ]; then
    echo "::error::migration $f version $v is <= the latest on main ($max_base); renumber above it — prefer \`sqlx migrate add <name>\` which uses a full UTC timestamp"
    fail=1
  else
    echo "OK: new migration $f version $v is above the latest on main ($max_base)"
  fi
done <<< "$new_files"

if [ "$fail" -ne 0 ]; then
  echo ""
  echo "Migration version-collision guard FAILED."
  echo "Two branches that both hand-number a daily YYYYMMDD000001 will collide and"
  echo "crash the service on boot (sqlx checksum mismatch). Renumber your new"
  echo "migration above $max_base, or recreate it with \`sqlx migrate add <name>\`."
  exit 1
fi

echo "Migration version-collision guard passed."
