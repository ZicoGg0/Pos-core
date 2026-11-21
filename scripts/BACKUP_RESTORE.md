# Personal OS — Backups & Restore

Two scripts in this folder:

- `pg-backup.sh` — nightly gzipped `pg_dump` of the whole database (tasks, jobs, study,
  communication **including chat media stored as bytea**, finance, everything), with retention,
  logging, verification, and a single-run lock. Uses only `pg_dump` (free).
- `pg-restore.sh` — restores a backup. **Safe by default**: restores into an isolated
  `pos_restore_test` database so you can verify a backup without touching live data.

Both assume the DB runs under Docker Compose in `~/pos-core` as service `db` (user/db `pos`).
Override with `POS_COMPOSE_DIR`, `POS_DB_SERVICE`, `POS_DB_USER`, `POS_DB_NAME`,
`POS_BACKUP_DIR` (default `~/pos-backups`), `POS_RETENTION_DAYS` (default 14).

## One-time setup (on the VM)

```bash
# 1) make sure the scripts are present and executable (they ship in the build)
chmod +x ~/pos-core/scripts/pg-backup.sh ~/pos-core/scripts/pg-restore.sh

# 2) run once by hand to confirm it works and see where dumps land
~/pos-core/scripts/pg-backup.sh
ls -lh ~/pos-backups        # -> pos-YYYY-mm-dd-HHMM.sql.gz  (mode 600) + backup.log

# 3) schedule it nightly at 03:17 (odd minute avoids the top-of-hour rush)
( crontab -l 2>/dev/null; echo '17 3 * * * /home/ubuntu/pos-core/scripts/pg-backup.sh >/dev/null 2>&1' ) | crontab -
crontab -l                  # verify the line is there
```

The script logs every run (success or failure) to `~/pos-backups/backup.log`.

## Verify a backup actually restores (do this occasionally)

```bash
~/pos-core/scripts/pg-restore.sh ~/pos-backups/pos-2026-11-21-0317.sql.gz
# -> rebuilds database "pos_restore_test" and prints row counts. Live data is untouched.
# Drop the test db when done:
cd ~/pos-core && docker compose exec -T db psql -U pos -d postgres -c 'DROP DATABASE IF EXISTS pos_restore_test;'
```

A backup is only trustworthy once you've seen it restore. The counts should look sane
(users, tasks, messages, media, finance_transactions).

## Disaster recovery — restore OVER the live database

Only when you really mean to replace live data (data loss / corruption):

```bash
cd ~/pos-core
docker compose stop api                       # stop writers first
./scripts/pg-restore.sh ~/pos-backups/<chosen-backup>.sql.gz --target live
#   ^ it will ask you to type the database name to confirm
docker compose start api
curl -s localhost:8080/health                 # -> ok
```

## Off-site copies (recommended)

These dumps live on the **same VM disk** as the database, so they protect against bad
migrations, fat-finger deletes, and app bugs — but **not** against losing the VM/disk itself.
For real safety, copy dumps off the box periodically (all free), e.g. from your own machine:

```powershell
scp -i $HOME\Downloads\oracle.key ubuntu@84.235.231.171:~/pos-backups/pos-*.sql.gz $HOME\Downloads\pos-backups\
```

or set up `rclone` to a free remote and add one line to cron. The on-VM retention keeps
`POS_RETENTION_DAYS` (default 14) days; off-site copies you keep as long as you like.
