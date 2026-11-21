#!/usr/bin/env bash
# Nightly PostgreSQL backup for Personal OS (pos-core).
# Dumps the whole database (including chat media stored as bytea) to a gzipped file
# on the host, with retention, logging, and a single-run lock. Uses only pg_dump.
#
# Run from cron, e.g. (3:17am daily):
#   17 3 * * * /home/ubuntu/pos-core/scripts/pg-backup.sh >/dev/null 2>&1
#
# Configurable via environment (sane defaults for the Oracle VM deployment):
#   POS_COMPOSE_DIR   dir holding docker-compose.yml   (default: ~/pos-core)
#   POS_DB_SERVICE    compose service name for Postgres (default: db)
#   POS_DB_USER       (default: pos)   POS_DB_NAME (default: pos)
#   POS_BACKUP_DIR    where dumps are written          (default: ~/pos-backups)
#   POS_RETENTION_DAYS keep dumps newer than N days    (default: 14)
#   POS_MIN_FREE_MB   refuse to run below this free MB  (default: 300)
#   POS_DB_EXEC       override the "run a pg tool in the db container" command
#                     (testing only; default uses docker compose exec)
set -euo pipefail

COMPOSE_DIR="${POS_COMPOSE_DIR:-$HOME/pos-core}"
DB_SERVICE="${POS_DB_SERVICE:-db}"
DB_USER="${POS_DB_USER:-pos}"
DB_NAME="${POS_DB_NAME:-pos}"
BACKUP_DIR="${POS_BACKUP_DIR:-$HOME/pos-backups}"
RETENTION_DAYS="${POS_RETENTION_DAYS:-14}"
MIN_FREE_MB="${POS_MIN_FREE_MB:-300}"
PREFIX="pos"                    # dump files: pos-YYYY-mm-dd-HHMM.sql.gz
LOG="$BACKUP_DIR/backup.log"
LOCK="$BACKUP_DIR/.backup.lock"

mkdir -p "$BACKUP_DIR"
chmod 700 "$BACKUP_DIR" 2>/dev/null || true

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') | $*" | tee -a "$LOG" >&2; }

# Run a postgres CLI tool inside the db container (overridable for tests).
db_exec() {
  if [ -n "${POS_DB_EXEC:-}" ]; then
    # shellcheck disable=SC2086
    $POS_DB_EXEC "$@"
  else
    ( cd "$COMPOSE_DIR" && docker compose exec -T "$DB_SERVICE" "$@" )
  fi
}

# Single-run lock so an overrunning backup never overlaps the next cron tick.
exec 9>"$LOCK"
if ! flock -n 9; then
  log "SKIP: another backup is already running"
  exit 0
fi

# Disk-space guard (don't fill the VM and take the app down).
FREE_MB=$(df -Pm "$BACKUP_DIR" | awk 'NR==2{print $4}')
if [ "${FREE_MB:-0}" -lt "$MIN_FREE_MB" ]; then
  log "ERROR: only ${FREE_MB}MB free in $BACKUP_DIR (need >= ${MIN_FREE_MB}MB) — skipping"
  exit 1
fi

STAMP="$(date '+%Y-%m-%d-%H%M')"
OUT="$BACKUP_DIR/${PREFIX}-${STAMP}.sql.gz"
TMP="${OUT}.partial"

log "START backup -> $OUT (free ${FREE_MB}MB, retention ${RETENTION_DAYS}d)"

# Dump straight to a temp file; only promote it to the final name once verified,
# so a crashed/partial dump is never mistaken for a good backup.
if ! db_exec pg_dump -U "$DB_USER" --no-owner --no-privileges "$DB_NAME" | gzip -c > "$TMP"; then
  log "ERROR: pg_dump failed"
  rm -f "$TMP"
  exit 1
fi

# Verify: gzip integrity, non-trivial size, and that it looks like a real dump.
if ! gzip -t "$TMP" 2>/dev/null; then
  log "ERROR: dump failed gzip integrity check"; rm -f "$TMP"; exit 1
fi
BYTES=$(wc -c < "$TMP")
if [ "$BYTES" -lt 1000 ]; then
  log "ERROR: dump suspiciously small (${BYTES} bytes)"; rm -f "$TMP"; exit 1
fi
if ! gzip -dc "$TMP" | grep -q 'PostgreSQL database dump'; then
  log "ERROR: dump missing expected header"; rm -f "$TMP"; exit 1
fi

mv "$TMP" "$OUT"
chmod 600 "$OUT"
log "OK backup verified: $(du -h "$OUT" | cut -f1) ($BYTES bytes gzipped)"

# Retention: delete only OUR dumps older than N days. Never touches other files.
DELETED=$(find "$BACKUP_DIR" -maxdepth 1 -type f -name "${PREFIX}-*.sql.gz" -mtime +"$RETENTION_DAYS" -print -delete | wc -l)
log "retention: removed $DELETED dump(s) older than ${RETENTION_DAYS} days"

REMAIN=$(find "$BACKUP_DIR" -maxdepth 1 -type f -name "${PREFIX}-*.sql.gz" | wc -l)
log "DONE: $REMAIN dump(s) retained"
