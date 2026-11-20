# pos-core — Personal OS core API (Rust + Postgres)

Foundation for the personal system. n8n handles schedules/integrations/AI;
this service owns the data and the rules.

## Run on the Oracle VM
    cp .env.example .env            # fill in the secrets (openssl rand -hex 32)
    docker network create n8n_net   # n8n must be attached: docker network connect n8n_net n8n
    docker compose up -d --build
    curl localhost:8080/health      # -> ok
HTTPS: add a site block to your existing Caddy that does `reverse_proxy localhost:8080`.

## Update the UI later
Unzip over the folder (`unzip -o pos-core.zip -d ~`; your .env is untouched). The `web/` folder is
mounted into the container, so UI changes show on the next reload. Rust or migration changes need
`docker compose up -d --build`.

## Auth
- App: `POST /auth/login {email,password}` -> `{token}` (30-day JWT), send `Authorization: Bearer <token>`.
- n8n: send `Authorization: Bearer <SERVICE_API_KEY>` (HTTP Request node, header auth credential).

## Endpoints
| Method | Path | Notes |
|---|---|---|
| GET | /health | no auth |
| POST | /auth/login | |
| GET / POST | /tasks | GET filters: `?status=todo&area=jobs` |
| PATCH / DELETE | /tasks/:id | `status: "done"` stamps `completed_at` |
| GET / POST | /jobs | POST dedups on `url`; below-floor pay is filed as `skipped`. GET `?status=&min_score=` |
| PATCH | /jobs/:id | status/score/notes; `applied` creates a 5-day follow-up task |
| GET / POST | /checkins | POST upserts on (day, kind); GET `?from=&to=` |

Areas: daily, health, jobs, study, finance, comms, fun. Statuses: todo, doing, done. Priority 1 (high) to 4.

## Next modules add migrations
`migrations/0002_*.sql` etc. Never edit an applied migration.

Pay floors (env, optional): MIN_HOURLY=15, MIN_MONTHLY=5000, MIN_YEARLY=50000. Compared within the listing's own period.

## AI picks (n8n)
The API triggers two n8n webhooks (same bearer key as the service key): `/webhook/fun-pick` (new picks, and an instant
replacement whenever you press "Not for me") and `/webhook/health-suggest` (today's meal and exercise ideas).
n8n is reached at `N8N_BASE_URL` (default `http://n8n:5678`). Both n8n workflows must be published for the webhooks to answer.

## Communication, sign-up and pairing
- `POST /auth/signup` creates a **member** account (name, email, password of 8+ characters). Members can only use Communication; every owner endpoint returns 403 for them, and the n8n service key is not accepted on `/comm`.
- Control sign-up in `.env`: `ALLOW_SIGNUP=false` closes it, `MAX_USERS=50` caps total accounts, `OWNER_NAME` sets your display name.
- Pairing: one person taps **Pair, Get a code** (8 characters, single use, 15 minutes). The other enters it. A chat then exists between the two accounts. Ten wrong guesses per 15 minutes locks that account's attempts.
- Offline: messages, photos, files and voice notes you send without a connection wait in the phone's local storage and send automatically when it reconnects. Chats you already loaded stay readable offline. This is queue-and-sync, not phone-to-phone messaging with no internet at all.
- Media (10 MB per file) is stored in Postgres, so your normal database backup covers it.
- Voice notes need the site served over HTTPS (your Caddy domain already is).
- Deploy the update: copy the new files up, then `sudo docker compose up -d --build`. Migrations 0003 to 0007 run on start.

## AI assistant
- **Assistant** in the sidebar is a chat with an AI that can read and change your tasks, jobs, check-ins, fun picks, health logs and study plan. It cannot delete anything.
- Clear instructions ("add a task to call the bank tomorrow at 10") are done straight away. Goals ("I want to study Rust") get a question first ("Should I add that to your study schedule?") and nothing changes until you say yes.
- It runs in n8n as the workflow **Personal OS - Assistant** (webhook `/webhook/assistant`). Gemini answers first (free), Claude is the backup. To change models, edit the two model nodes in that workflow. The same pattern is used by the Fun Picker and Health Suggestions workflows.
- Finance and Communication are not connected to the assistant yet.
