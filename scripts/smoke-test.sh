#!/usr/bin/env bash
# Integration / regression suite for the pos-core API.
# Assumes the API is already running and reachable at $BASE with a fresh database
# (migrations applied, single owner bootstrapped). Exits non-zero if any check fails.
#
#   BASE              base URL            (default http://127.0.0.1:8080)
#   SERVICE_API_KEY   the service key     (required for the service-key checks)
#   OWNER_EMAIL / OWNER_PASSWORD          (default owner@test.local / ownerpass123)
set -uo pipefail
BASE="${BASE:-http://127.0.0.1:8080}"
OWNER_EMAIL="${OWNER_EMAIL:-owner@test.local}"
OWNER_PASSWORD="${OWNER_PASSWORD:-ownerpass123}"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); echo "  ok   $1"; }
no(){ FAIL=$((FAIL+1)); echo "  FAIL $1 -> ${2:-}"; }
j(){ python3 -c "import sys,json;print(json.load(sys.stdin)$1)" 2>/dev/null; }
code(){ curl -s -o /dev/null -w '%{http_code}' "$@"; }
uuid(){ python3 -c "import uuid;print(uuid.uuid4())"; }
check(){ local d="$1"; shift; if [ "$1" = "$2" ]; then ok "$d"; else no "$d" "$1 != $2"; fi; }

echo "# auth + boundaries"
TOK=$(curl -s $BASE/auth/login -H 'content-type: application/json' -d "{\"email\":\"$OWNER_EMAIL\",\"password\":\"$OWNER_PASSWORD\"}" | j "['token']")
[ -n "$TOK" ] && ok "owner login" || no "owner login" "no token"
AH="authorization: Bearer $TOK"
check "wrong password -> 401" "$(code $BASE/auth/login -H 'content-type: application/json' -d "{\"email\":\"$OWNER_EMAIL\",\"password\":\"nope\"}")" 401
check "/auth/me role owner" "$(curl -s $BASE/auth/me -H "$AH" | j "['role']")" owner
if [ -n "${SERVICE_API_KEY:-}" ]; then
  check "service key -> owner route 200" "$(code $BASE/tasks -H "authorization: Bearer $SERVICE_API_KEY")" 200
  check "service key -> /comm 401" "$(code $BASE/comm/conversations -H "authorization: Bearer $SERVICE_API_KEY")" 401
fi

echo "# tasks"
TID=$(curl -s $BASE/tasks -H "$AH" -H 'content-type: application/json' -d '{"title":"t1","priority":1}' | j "['id']")
[ -n "$TID" ] && ok "task create" || no "task create"
check "task done stamps completed_at" "$(curl -s -X PATCH $BASE/tasks/$TID -H "$AH" -H 'content-type: application/json' -d '{"status":"done"}' | python3 -c "import sys,json;print(json.load(sys.stdin)['completed_at'] is not None)")" True
check "task delete 204" "$(code -X DELETE $BASE/tasks/$TID -H "$AH")" 204
check "task bad area 400" "$(code $BASE/tasks -H "$AH" -H 'content-type: application/json' -d '{"title":"x","area":"zz"}')" 400

echo "# jobs"
JID=$(curl -s $BASE/jobs -H "$AH" -H 'content-type: application/json' -d '{"source":"m","title":"J","url":"https://x.test/j1","pay_min":40,"pay_period":"hour"}' | j "['id']")
check "job dedup" "$(curl -s $BASE/jobs -H "$AH" -H 'content-type: application/json' -d '{"source":"m","title":"d","url":"https://x.test/j1"}' | j "['duplicate']")" True
check "below floor skipped" "$(curl -s $BASE/jobs -H "$AH" -H 'content-type: application/json' -d '{"source":"m","title":"l","url":"https://x.test/lo","pay_max":5,"pay_period":"hour"}' | j "['status']")" skipped
curl -s -X PATCH $BASE/jobs/$JID -H "$AH" -H 'content-type: application/json' -d '{"status":"applied"}' >/dev/null
curl -s "$BASE/tasks?area=jobs" -H "$AH" | grep -q "Follow up" && ok "applied creates follow-up task" || no "follow-up task"

echo "# checkins + study/leitner"
curl -s $BASE/checkins -H "$AH" -H 'content-type: application/json' -d '{"day":"2026-11-20","kind":"morning","mood":4}' | grep -q morning && ok "checkin upsert" || no "checkin"
SID=$(curl -s $BASE/study/subjects -H "$AH" -H 'content-type: application/json' -d '{"name":"S'$RANDOM'"}' | j "['id']")
RID=$(curl -s $BASE/study/reviews -H "$AH" -H 'content-type: application/json' -d "{\"subject_id\":\"$SID\",\"prompt\":\"p\"}" | j "['id']")
EXP=(3 7 14 30 60); i=0
for b in 1 2 3 4 5; do
  R=$(curl -s $BASE/study/reviews/$RID/answer -H "$AH" -H 'content-type: application/json' -d '{"remembered":true,"on":"2026-11-20"}')
  D=$(python3 -c "import datetime;print((datetime.date(2026,11,20)+datetime.timedelta(days=${EXP[$i]})).isoformat())")
  check "leitner box $b due +${EXP[$i]}d" "$(echo "$R"|j "['box']")-$(echo "$R"|j "['due_on']")" "$b-$D"; i=$((i+1))
done
check "leitner forget->box0" "$(curl -s $BASE/study/reviews/$RID/answer -H "$AH" -H 'content-type: application/json' -d '{"remembered":false,"on":"2026-11-20"}' | j "['box']")" 0

echo "# health + fun"
curl -s -X PUT $BASE/health/weights/2026-11-20 -H "$AH" -H 'content-type: application/json' -d '{"kg":72.5}' | grep -q 72.5 && ok "weight put" || no "weight"
curl -s $BASE/health/meals -H "$AH" -H 'content-type: application/json' -d '{"slot":"lunch","description":"beans"}' | grep -q beans && ok "meal" || no "meal"
curl -s -X PUT $BASE/fun/profile -H "$AH" -H 'content-type: application/json' -d '{"hobbies":["x"],"consumes":[],"has_partner":false}' | grep -q '"hobbies"' && ok "fun profile" || no "fun profile"

echo "# finance"
check "income create" "$(curl -s $BASE/finance/transactions -H "$AH" -H 'content-type: application/json' -d '{"kind":"income","amount":500000,"category":"salary"}' | python3 -c "import sys,json;print(json.load(sys.stdin)['id']!='')")" True
check "bad kind 400" "$(code $BASE/finance/transactions -H "$AH" -H 'content-type: application/json' -d '{"kind":"zz","amount":1}')" 400
check "bad currency 400" "$(code $BASE/finance/transactions -H "$AH" -H 'content-type: application/json' -d '{"kind":"income","amount":1,"currency":"NAIRA"}')" 400
BAL=$(curl -s $BASE/finance/overview -H "$AH" | j "['balance']"); check "overview balance 500000" "$(python3 -c "print(int(float('$BAL')))")" 500000
curl -s $BASE/finance/budgets -H "$AH" -H 'content-type: application/json' -d '{"category":"food","limit_amount":60000}' | grep -q 60000 && ok "budget upsert" || no "budget"
DID=$(curl -s $BASE/finance/debts -H "$AH" -H 'content-type: application/json' -d '{"creditor":"Bank","original_amount":100000}' | j "['id']")
check "repay to paid" "$(curl -s $BASE/finance/debts/$DID/repayments -H "$AH" -H 'content-type: application/json' -d '{"amount":100000}' | j "['status']")" paid
check "repay bad debt 404" "$(code $BASE/finance/debts/$(uuid)/repayments -H "$AH" -H 'content-type: application/json' -d '{"amount":1}')" 404
check "insights 3 months" "$(curl -s "$BASE/finance/insights?months=3" -H "$AH" | python3 -c "import sys,json;print(len(json.load(sys.stdin)['months']))")" 3

echo "# settings + password"
check "prefs defaults NGN" "$(curl -s $BASE/settings/prefs -H "$AH" | j "['currency']")" NGN
check "prefs bad date_format 400" "$(code -X PUT $BASE/settings/prefs -H "$AH" -H 'content-type: application/json' -d '{"date_format":"ZZ"}')" 400
check "pw wrong current 400 (not 401)" "$(code $BASE/auth/password -H "$AH" -H 'content-type: application/json' -d '{"current":"nope","new_password":"abcdefgh"}')" 400

echo "# communication + isolation"
A=$(curl -s $BASE/auth/signup -H 'content-type: application/json' -d '{"email":"sa'$RANDOM'@t.local","password":"passpass1","name":"A"}'); AT=$(echo "$A"|j "['token']")
check "signup role member" "$(echo "$A"|j "['role']")" member
BT=$(curl -s $BASE/auth/signup -H 'content-type: application/json' -d '{"email":"sb'$RANDOM'@t.local","password":"passpass1","name":"B"}'|j "['token']")
CT=$(curl -s $BASE/auth/signup -H 'content-type: application/json' -d '{"email":"sc'$RANDOM'@t.local","password":"passpass1","name":"C"}'|j "['token']")
check "member -> owner route 403" "$(code $BASE/tasks -H "authorization: Bearer $AT")" 403
check "member -> finance 403" "$(code $BASE/finance/overview -H "authorization: Bearer $AT")" 403
CODE=$(curl -s -X POST $BASE/comm/pair/code -H "authorization: Bearer $AT"|j "['code']")
check "pair code 8 chars" "${#CODE}" 8
CONV=$(curl -s -X POST $BASE/comm/pair/join -H "authorization: Bearer $BT" -H 'content-type: application/json' -d "{\"code\":\"$CODE\"}"|j "['id']")
[ -n "$CONV" ] && ok "pair join" || no "pair join"
check "pair reuse 400" "$(code -X POST $BASE/comm/pair/join -H "authorization: Bearer $CT" -H 'content-type: application/json' -d "{\"code\":\"$CODE\"}")" 400
MID=$(uuid)
check "msg create 201" "$(code $BASE/comm/conversations/$CONV/messages -H "authorization: Bearer $AT" -H 'content-type: application/json' -d "{\"id\":\"$MID\",\"kind\":\"text\",\"body\":\"hi\"}")" 201
check "msg idempotent 200" "$(code $BASE/comm/conversations/$CONV/messages -H "authorization: Bearer $AT" -H 'content-type: application/json' -d "{\"id\":\"$MID\",\"kind\":\"text\",\"body\":\"hi\"}")" 200
check "non-member reads conv 404" "$(code $BASE/comm/conversations/$CONV/messages -H "authorization: Bearer $CT")" 404
MEDIA=$(uuid); printf 'bytes' > /tmp/sm.bin
check "media upload 201" "$(code -X PUT $BASE/comm/media/$MEDIA -H "authorization: Bearer $AT" -H "x-conversation: $CONV" -H 'content-type: text/plain' --data-binary @/tmp/sm.bin)" 201
check "non-member media 404" "$(code $BASE/comm/media/$MEDIA -H "authorization: Bearer $CT")" 404
rm -f /tmp/sm.bin

echo
echo "================  PASS=$PASS  FAIL=$FAIL  ================"
[ "$FAIL" -eq 0 ]
