#!/usr/bin/env bash
# Restore a Personal OS backup produced by pg-backup.sh.
#
# SAFE BY DEFAULT: restores into an ISOLATED test database so you can verify a
# backup without touching live data. Only restores over the real database when
# you explicitly pass --target live (and confirm).
#
# Usage:
#   scripts/pg-restore.sh <backup.sql.gz>                 # -> restores into pos_restore_test
#   scripts/pg-restore.sh <backup.sql.gz> --target live   # -> restores into the LIVE db (asks first)
#   scripts/pg-restore.sh <backup.sql.gz> --target NAME   # -> restores into database NAME
#
# Same POS_* env overrides as pg-backup.sh.
set -euo pipefail

COMPOSE_DIR="${POS_COMPOSE_DIR:-$HOME/pos-core}"
DB_SERVICE="${POS_DB_SERVICE:-db}"
DB_USER="${POS_DB_USER:-pos}"
LIVE_DB="${POS_DB_NAME:-pos}"

FILE="${1:-}"
TARGET_MODE="test"
if [ "${2:-}" = "--target" ]; then TARGET_MODE="${3:-}"; fi
[ -n "$FILE" ] && [ -f "$FILE" ] || { echo "usage: $0 <backup.sql.gz> [--target test|live|NAME]"; exit 1; }

case "$TARGET_MODE" in
  test|"") TARGET="pos_restore_test" ; ISOLATED=1 ;;
  live)    TARGET="$LIVE_DB"         ; ISOLATED=0 ;;
  *)       TARGET="$TARGET_MODE"     ; ISOLATED=0 ;;
esac

db_exec() {
  if [ -n "${POS_DB_EXEC:-}" ]; then $POS_DB_EXEC "$@";
  else ( cd "$COMPOSE_DIR" && docker compose exec -T "$DB_SERVICE" "$@" ); fi
}

echo "Backup file : $FILE"
echo "Restore into: $TARGET  ($([ "$ISOLATED" = 1 ] && echo 'isolated test db' || echo 'EXISTING db'))"

if [ "$TARGET" = "$LIVE_DB" ]; then
  echo "!! This will DROP and recreate the LIVE database '$LIVE_DB'. All current data is replaced."
  read -r -p "Type the database name ('$LIVE_DB') to confirm: " ANS
  [ "$ANS" = "$LIVE_DB" ] || { echo "aborted."; exit 1; }
fi

# Recreate the target database fresh, then load the dump into it.
db_exec psql -U "$DB_USER" -d postgres -v ON_ERROR_STOP=1 \
  -c "DROP DATABASE IF EXISTS \"$TARGET\";" \
  -c "CREATE DATABASE \"$TARGET\" OWNER \"$DB_USER\";"

gzip -dc "$FILE" | db_exec psql -U "$DB_USER" -d "$TARGET" -v ON_ERROR_STOP=1 -q

echo "Restore complete. Row counts in '$TARGET':"
db_exec psql -U "$DB_USER" -d "$TARGET" -t -c \
  "SELECT 'users', count(*) FROM users UNION ALL SELECT 'tasks', count(*) FROM tasks UNION ALL SELECT 'messages', count(*) FROM messages UNION ALL SELECT 'media', count(*) FROM media UNION ALL SELECT 'finance_transactions', count(*) FROM finance_transactions ORDER BY 1;"
