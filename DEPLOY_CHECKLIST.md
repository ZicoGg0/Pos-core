# Personal OS — Production Deploy Checklist

Production: API `https://povgdh.duckdns.org` · n8n `https://d1n8n.duckdns.org` · Oracle ARM VM.
The API runs in Docker Compose at `~/pos-core`, on the external `n8n_net` network; n8n reaches it
at `http://api:8080`. The `web/` folder is bind-mounted (UI updates show on reload).

## 0. Before you deploy
- [ ] You have the **final build** zip (includes Finance, Settings, login rate-limit, backup
      scripts, updated PWA manifest + screenshots).
- [ ] Your existing `~/pos-core/.env` is intact (it is NOT in the zip; `unzip -o` preserves it).
- [ ] No new required env vars were added — the current `.env` is sufficient.

## 1. Back up first (production has only migrations 0001–0002; this deploy runs 0003–0009)
```bash
cd ~/pos-core
sudo docker compose exec -T db pg_dump -U pos pos | gzip > ~/pos-pre-deploy-$(date +%F-%H%M).sql.gz
ls -lh ~/pos-pre-deploy-*.sql.gz        # confirm non-trivial size; keep it
```

## 2. Deploy
```bash
# from your computer (PowerShell):
#   scp -i $HOME\Downloads\oracle.key $HOME\Downloads\pos-core.zip ubuntu@84.235.231.171:~
cd ~ && unzip -o pos-core.zip -d ~
cd ~/pos-core
sudo docker compose up -d --build
```

## 3. Verify
- [ ] `curl -s localhost:8080/health` → `ok`
- [ ] `sudo docker compose logs --tail=50 api` → migrations applied, no errors
- [ ] Open the app, hard-refresh once (service worker is now `pos-v9`)
- [ ] **Finance** and **Settings** tabs appear and load
- [ ] Settings → change-password works; wrong current password shows an error (not a logout)
- [ ] Existing data (tasks/jobs/chats/media) all intact
- [ ] Confirm migrations: `sudo docker compose exec -T db psql -U pos -d pos -c "SELECT version FROM _sqlx_migrations ORDER BY version;"` → 1..9

## 4. Nightly backups (one-time)
Follow `scripts/BACKUP_RESTORE.md`:
```bash
~/pos-core/scripts/pg-backup.sh          # run once by hand, confirm a dump appears
( crontab -l 2>/dev/null; echo '17 3 * * * /home/ubuntu/pos-core/scripts/pg-backup.sh >/dev/null 2>&1' ) | crontab -
```
- [ ] Backup ran and produced `~/pos-backups/pos-*.sql.gz`
- [ ] Cron line installed

## 5. Reconnect Gmail in n8n (fixes the failing Job Finder)
The Job Finder's final email node fails with "Gmail account needs to be reconnected" (OAuth token
expired). This is the only reason its recent runs errored; job fetching/scoring/posting works.
- [ ] In n8n (`https://d1n8n.duckdns.org`) → **Credentials** → open **"Gmail account"**
- [ ] Click **Reconnect** / re-run the Google OAuth consent, save
- [ ] Open **Personal OS - Job Finder** → **Execute workflow** once → confirm it finishes green
- [ ] (Optional robustness, your call: set the "Email No Matches" node to *Continue On Fail* so a
      notification hiccup never marks the whole run as failed.)

## 6. Assistant Finance tools (after the API is live)
The in-app assistant currently has 23 tools (no Finance). Once this build is deployed, the Finance
endpoints exist in prod and I can add Finance tools to the **Personal OS - Assistant** workflow:
`finance_overview`, `record_income`, `record_expense`, `list_transactions`, `list_debts`,
`record_repayment`, `summarize_spending` — with confirmation required before money-moving actions,
matching the existing ask-first pattern. The assistant system prompt will be updated to connect
Finance (and Communication stays excluded).
- [ ] Tell me when the API is deployed and I'll add + publish these via the n8n tools, then test
      read + write + ask-first against the live server.

## 7. Rollback (if needed)
```bash
cd ~/pos-core
docker compose stop api
./scripts/pg-restore.sh ~/pos-pre-deploy-<stamp>.sql.gz --target live   # asks for confirmation
docker compose start api
```
(Schema changes 0003–0009 are additive, so a rollback is only needed for unrelated problems.)
