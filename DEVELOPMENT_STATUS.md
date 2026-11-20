# Personal OS — Development Status

**Last updated:** 2026-11-20
**Current phase:** PHASE 6 — Settings COMPLETE & tested. (Finance = Phase 5 done.)
Next: backups (Phase 7), backend test suite, UI polish, assistant Finance tools (at deploy).
**Lead engineer note:** This file is the single source of truth for project state.
Update it after each completed phase.

---

## 0. Environment & access reality (READ FIRST)

What I (the build environment) can and cannot reach shapes every verification task.

| Resource | Reachable from here? | How |
|---|---|---|
| Source code (Rust + web) | ✅ | Local git repo, pushed to `github.com/ZicoGg0/pos-core` |
| Rust toolchain (cargo 1.97) | ✅ | Local |
| Docker + Compose | ✅ | Local — can run full Postgres + API stack for real integration tests |
| **n8n instance** (`d1n8n.duckdns.org`) | ✅ | **n8n MCP is connected to the real instance** — workflow IDs match |
| Production API (`povgdh.duckdns.org`) | ⚠️ public HTTPS only | Can hit unauthenticated `/health`; no owner token here |
| Production Postgres / the Oracle VM | ❌ | No SSH key, no Remote Control connection in this session |
| Production data | ❌ (by policy) | Must not touch without explicit approval |

**Consequence:** backend logic, migrations, and API behaviour can be verified **locally** against a
throwaway Postgres (no production risk). n8n workflows can be **inspected and test-run via MCP**.
Anything requiring the production DB, the VM filesystem (e.g. installing a backup cron), or the
owner's live token must be prepared here and **handed to the owner to run**, or requires the owner
to attach the VM via Remote Control.

---

## 1. Completed functionality (exists in repo, logic verified by reading; runtime test pending)

### Backend (Rust / Axum 0.7 / SQLx 0.8 / Postgres)
- **Auth**: Argon2 password hashing; JWT (HS256, 30-day) with `role` claim; `issue_token`,
  `verify_password`. Constant-time compare for the service key.
- **Authorization model** (matches spec exactly):
  - `Authed` extractor → owner JWT **or** static service key → owner data routes.
  - `Member` extractor → any signed-in JWT (owner or member), **service key rejected** → `/comm` only.
  - Member JWTs get 403 on owner routes (role check).
- **Owner bootstrap**: first boot creates the single owner from `OWNER_EMAIL/PASSWORD/NAME`.
- **Tasks**: list (filter status/area), create, update (status→`done` stamps `completed_at`), delete.
- **Check-ins**: upsert on `(day, kind)`, range list.
- **Jobs**: create (dedup on `url`, below-pay-floor auto-`skipped`, `queued` support), list
  (filter status/min_score), update (→`applied` creates a 5-day follow-up task).
- **Fun**: profile upsert (single row), picks list/create/patch, `request` + reject fire n8n
  `/webhook/fun-pick`.
- **Health**: weights (upsert/delete by day), workouts (CRUD-ish), meals, daily AI suggestions
  (separate table `health_suggestions`), `request` fires `/webhook/health-suggest`.
- **Study**: subjects, sessions, reviews, **Leitner** spaced repetition server-side
  (boxes 0–5, intervals `[1,3,7,14,30,60]`, forget→box 0).
- **Communication**: pairing codes (8-char, single-use, 15-min TTL, 10-guess limiter),
  1:1 conversations, idempotent messages (client UUID) + seq polling, media in Postgres
  (10 MB cap, idempotent upload, sandboxed `Content-Security-Policy` on fetch).
- **Assistant**: `/assistant/chat` forwards to n8n `/webhook/assistant` (owner-only), trims
  history to last 12 turns, 120s timeout, graceful errors.

### Frontend (vanilla single-file PWA, `web/index.html` ~69 KB)
- Views: Today, Tasks, Jobs, Health, Fun, Study, Communication, Assistant.
- Light/dark theme toggle (`data-theme`, persisted in `localStorage`).
- Service worker `pos-v8`: network-first, **API prefixes excluded from cache** (correct).
- Communication offline outbox (IndexedDB), pairing UI, media/voice capture.
- Members are auto-pinned to the Communication view.
- No service key or secret present in frontend (verified).

### n8n workflows (confirmed present & published via MCP)
- Personal OS - Assistant (`N1s4Gx3tOoQwcMLj`) — 29 nodes, published.
- Personal OS - Health Suggestions (`xmvDIdIBmH0JXhf2`) — published.
- Personal OS - Fun Picker (`1vNQpd1BTzUAjZIS`) — published.
- Personal OS - Job Finder (`6N4csWvApLl7ExF8`) — published.

---

## 2. Partially completed functionality
- **Settings**: only a theme toggle exists. No account/password/preferences/session UI, no
  `user_prefs` persistence. (Spec §8 wants a full Settings module.)
- **AI Assistant write path**: wired end-to-end, but per spec only **read** tools were verified
  against the live server. Ask-first-for-study behaviour and all **write** tools are UNVERIFIED.
- **PWA/APK readiness**: manifest has name/short_name/icons(192,512 maskable)/theme/start_url/
  display=standalone. **Missing** for a clean PWABuilder/TWA pass: screenshots, richer icon set,
  `id`, `scope`, possibly `shortcuts`. Offline shell works; needs validation.

---

## 3. Missing functionality (to build)
- **Finance module** (§7) — entirely absent. Needs: schema (tables), API routes, frontend view,
  assistant tools. `tasks.area` already permits `'finance'`, so it was anticipated.
  **Schema change → requires approval before implementing (constraint 15).**
- **Settings module** (§8) — build supported settings only; likely needs a small `user_prefs`
  table (**schema change → approval**).
- **Nightly database backups** (§9) — no backup script/cron anywhere. Runs on the VM →
  needs owner action or Remote Control.
- **Automated tests** (§13) — **zero** tests in the repo (no `tests/`, no `#[test]`).
- **APK packaging assets** (§15) — screenshots and packaging config for PWABuilder.

---

## 4. Known bugs / issues
1. **Job Finder workflow failing**: 3 of 4 recent runs failed (last `execution:2055`, ~5h ago).
   Root cause not yet diagnosed — needs MCP execution inspection. (Spec §12 notes Gemini/Mistral/
   Groq free-tier rate limits as a recurring cause.)
2. **Assistant latency 15–60s** (Gemini). Spec §6 asks for optimisation without paid services.
3. **No login rate limiting**: only the pairing-code `/comm/pair/join` path is throttled.
   `/auth/login` has no brute-force protection. (Security §11.)
4. **Anthropic fallback has no credit** — configured but non-operational; must not be assumed live.
5. **Gmail credential** in n8n needs reconnecting (owner action; §12).

(No definite logic bugs found in the Rust by reading; to be confirmed by local integration tests.)

---

## 5. Deployment discrepancies
- **Production runs an older API**: migrations **0003–0007 and the newer module routes are not
  yet deployed** (per spec §4). Production likely only has tasks/jobs/checkins from 0001–0002.
- Deploying will run migrations 0003–0007 on the production DB on container start →
  **requires approval + a tested backup first** (constraints 15/16, §9).
- Compose: API binds `127.0.0.1:8080`, joins external `n8n_net`, service name `api` →
  n8n reaches it at `http://api:8080` ✓. Caddy reverse-proxies the public domain ✓ (external config).

---

## 6. Security concerns (for the §11 audit)
- `/auth/login` not rate-limited (brute-force risk) — **recommend fixing**.
- CORS `allow_origin(Any)` — acceptable (Bearer tokens, no cookies) but worth a conscious decision.
- JWT 30-day, no revocation/rotation, no session management (spec §8 wants "active session
  management where supported" — HS256 stateless tokens can't be individually revoked without a
  denylist; note as a limitation).
- File upload: MIME allow-list + 10 MB + sandbox CSP on serve ✓. Good.
- SQL: all queries parameterised (`bind`) ✓ — no injection surface found.
- Error handling hides internals (`AppError::Internal` logs, returns generic) ✓.
- **Boundaries confirmed intact**: owner-only data, member→comms-only, service-key-never-on-/comm,
  assistant owner-only. Do not weaken when adding Finance/Settings.

---

## 7. Recommended implementation order
Follows the spec's phases, front-loading the lowest-risk, highest-certainty work and gating every
production/schema action on approval:

1. **Phase 2** — Stand up the full stack **locally** (Postgres + API in Docker), run migrations
   0001–0007, smoke-test every route. Diagnose the Job Finder failure via n8n MCP. *(No prod risk.)*
2. **Phase 3** — Verify existing modules against the local stack; write a backend integration test
   suite (the missing §13 coverage) as the verification vehicle.
3. **Phase 4** — Verify Assistant tools via n8n MCP (read + **write** + ask-first), using the local
   API as the tool backend so no production data is touched.
4. **Phase 5** — **Finance**: present schema for approval → implement API → frontend → assistant
   tools → test. *(Approval gate.)*
5. **Phase 6** — Settings (propose `user_prefs` schema for approval).
6. **Phase 7** — Backups: write + test the script locally (dump/restore into a throwaway DB);
   hand the owner the cron install step for the VM.
7. **Phase 8** — UI/UX consistency + progressive loading + caching.
8. **Phase 9** — Security hardening (login rate limit) + full regression run.
9. **Phase 10** — Prepare deployment checklist; **await approval** to deploy to prod.
10. **Phase 11** — PWA validation + APK readiness report. **Stop before packaging.**

---

## Phase 5 — Finance (DONE, local)
- Migration `0008_finance.sql`: finance_transactions, finance_budgets, finance_debts,
  finance_repayments. Applied cleanly on local stack.
- API `src/routes/finance.rs` (owner-only): overview, transactions CRUD, budgets upsert+list+del,
  debts list/create/patch, repayments (decrements outstanding, flips to 'paid' at 0), insights.
- Frontend: new **Finance** view + nav (was "Soon"), overview strip, add-transaction form,
  recent list, insights bars + top-spending, budgets with progress, debts with inline repay.
  `/finance` added to SW cache-exclusion list.
- **Tested:** 20+ API checks pass (amounts exact, dedup/upsert, repay→paid, insights months,
  multi-currency, member→403); browser test (Playwright) renders with **0 console errors**,
  add-transaction works through the UI; 9-view desktop+mobile regression = 0 errors.
- **Not yet done (needs deploy coordination):** assistant Finance tools — they call
  `/finance/*` which won't exist in prod until the API is deployed. Prepared for the deploy step.

## Phase 6 — Settings (DONE, local)
- Migration `0009_prefs.sql`: `user_prefs` (per user: currency, date_format, timezone, theme).
- API: `GET/PUT /settings/prefs` (self-scoped via `Member`, get-or-create), and
  `POST /auth/password` (verify current → set new; **wrong current = 400, not 401**, so the app
  doesn't force a logout).
- Frontend: new **Settings** view + nav: Account (name/email/role + change-password form),
  Appearance (light/dark/system theme, synced to server), Preferences (currency/date format/
  timezone — stored and actually applied: currency drives new-transaction currency + display,
  date format drives Finance date rendering), honest Notifications/Privacy info, About (v1.0.0).
- **Tested:** 10 backend checks pass (defaults, validation, password round-trip, member self-
  scope); browser test renders Settings + Finance with **0 console errors**, theme toggle works,
  email resolves. SW cache bumped `pos-v8`→`pos-v9`, `/settings` excluded from cache.

## Pending approvals
- [x] Finance DB schema (migration 0008) — APPROVED, implemented.
- [x] Settings `user_prefs` schema (migration 0009) — APPROVED (prefs + password), implemented.
- [ ] Settings `user_prefs` schema (if needed) — blocks Phase 6.
- [ ] Any production migration / deploy — blocks Phase 10.
- [ ] Backup cron on the VM — needs owner action or Remote Control.

## Tests performed
- Static read of entire backend + schema + frontend.
- `cargo build --release` → **clean, exit 0**, 1m42s, no warnings.
- Local stack stood up (Postgres 16 + API binary). Migrations 0001–0007 **all applied**,
  17 tables created, owner bootstrap OK.
- **Route smoke tests — 40+ checks, all PASS:**
  - Auth: login ok/401, `/auth/me` role, service-key→owner routes, member→owner route **403**.
  - Tasks: create/list/update(done stamps completed_at)/delete/invalid-area-400.
  - Jobs: create, dedup-on-url, below-pay-floor→skipped, applied→applied_at + **5-day follow-up task**.
  - Checkins: upsert + overwrite on (day,kind).
  - Study/Leitner: box progression 1→2→3→4→5 with due dates **+3,+7,+14,+30,+60** exactly,
    cap at box 5, forget→box 0 due +1. **Correct.**
  - Health: weight/workout/meal/suggestion (AI suggestions in separate table — confirmed).
  - Fun: profile upsert, pick create/patch.
  - Communication: member signup (role=member), 8-char pair code, join→conversation,
    single-use code, idempotent message (201 then 200 on retry), peer read, **non-member→404
    isolation**, media upload/fetch, **non-member media→404**, sandbox CSP header, >10MB→400.
  - Boundaries: **service key→/comm = 401**, member→assistant = 403, assistant graceful-fails
    when n8n down.
- **Job Finder failure (execution:2055) diagnosed:** NOT a code bug — Gmail credential revoked.
  Fails only at the final "Email No Matches" notify node; job fetch/score/post path runs.
  → Owner must reconnect Gmail in n8n. Possible robustness fix (needs approval): set that email
  node to continue-on-fail so a notify failure doesn't error the whole run.
- **Assistant workflow verified (static):** 23 tools (tasks×3, jobs×2, checkins×2, fun×4,
  health×7, study×5), Gemini primary + Claude Haiku fallback, **ask-first logic present** in
  system prompt for study subjects/reviews/sessions/fun requests, Finance/Comm excluded, no delete.
  Tool endpoints match the verified API. Live write-tests deferred (they hit production).

## Verified module status (local)
| Module | API | Logic | Notes |
|---|---|---|---|
| Auth/roles | ✅ | ✅ | boundaries intact |
| Tasks | ✅ | ✅ | |
| Checkins | ✅ | ✅ | |
| Jobs | ✅ | ✅ | follow-up rule works |
| Study/Leitner | ✅ | ✅ | intervals exact |
| Health | ✅ | ✅ | |
| Fun | ✅ | ✅ | n8n fire is async |
| Communication | ✅ | ✅ | isolation + idempotency verified |
| Assistant | ✅ wiring | ⏳ live | structure verified; live write test needs approval |

## Next recommended task
Phase 5 — Finance. Schema proposed in `FINANCE_SCHEMA_PROPOSAL.md`; **awaiting approval** before
writing migration 0008. In parallel (no approval needed): backend integration test suite + backup
script (built & tested locally).
