#!/usr/bin/env bash
# End-to-end test against a running TalkOps + FreeSWITCH stack.
#
# Creates an admin, two extensions with one device each, registers device
# 21-1 with SIPp, calls it from 20-1 and checks the call record. Also checks
# that a wrong SIP password is rejected.
#
#   BASE=http://127.0.0.1:8080 SIP_HOST=192.168.1.10 tests/e2e/run.sh
set -euo pipefail

BASE=${BASE:-http://127.0.0.1:8080}
SIP_HOST=${SIP_HOST:-$(hostname -I | awk '{print $1}')}
SIP_PORT=${SIP_PORT:-5060}
DIR=$(cd "$(dirname "$0")/../sipp" && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"; kill $(jobs -p) 2>/dev/null || true' EXIT

log() { printf '\n==> %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

COOKIES="$WORK/cookies"
CSRF=""
api() { # method path [json]
    local args=(-sS -f -b "$COOKIES" -c "$COOKIES" -H 'X-Requested-With: TalkOps' -H "X-CSRF-Token: $CSRF" -X "$1")
    [ $# -ge 3 ] && args+=(-H 'Content-Type: application/json' -d "$3")
    curl "${args[@]}" "$BASE$2"
}

log "waiting for TalkOps and FreeSWITCH"
for _ in $(seq 1 90); do
    if curl -sf "$BASE/api/v1/status" | jq -e '.database.ok and .freeswitch.ok' >/dev/null 2>&1; then break; fi
    sleep 2
done
curl -sf "$BASE/api/v1/status" | jq -e '.freeswitch.ok' >/dev/null || fail "FreeSWITCH not connected"

log "first-run setup"
if curl -sf "$BASE/api/v1/setup" | jq -e '.needs_setup' >/dev/null; then
    CSRF=$(api POST /api/v1/setup '{"username":"admin","display_name":"Admin","password":"e2e-password-1"}' | jq -r .csrf_token)
else
    CSRF=$(api POST /api/v1/auth/login '{"username":"admin","password":"e2e-password-1"}' | jq -r .csrf_token)
fi

log "extensions and devices"
device() { # number -> "username password"
    local ext id
    ext=$(api POST /api/v1/extensions "{\"number\":\"$1\",\"display_name\":\"E2E $1\"}") || fail "create extension $1"
    id=$(echo "$ext" | jq -r .id)
    api POST "/api/v1/extensions/$id/devices" '{"name":"SIPp","kind":"softphone"}' | jq -r '"\(.sip_username) \(.sip_password)"'
}
read -r CALLER CALLER_PW <<<"$(device 20)"
read -r CALLEE CALLEE_PW <<<"$(device 21)"
echo "caller=$CALLER callee=$CALLEE"

log "wrong password is rejected"
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/bad_password.xml" -s "$CALLEE" -au "$CALLEE" -ap "not-the-password" \
    -m 1 -p 5091 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/bad.log" >/dev/null \
    || { cat "$WORK/bad.log" 2>/dev/null; fail "wrong password was not rejected"; }

log "callee registers (calls are answered by a SIPp UAS on port 5094)"
sipp -sn uas -p 5094 -i "$SIP_HOST" -m 1 -min_rtp_port 16000 -max_rtp_port 16050 -timeout 90 -timeout_error \
    -trace_err -error_file "$WORK/callee.log" >"$WORK/callee.out" 2>&1 &
CALLEE_PID=$!
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/register.xml" -set contact_port 5094 -s "$CALLEE" -au "$CALLEE" -ap "$CALLEE_PW" \
    -m 1 -p 5092 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/register.log" >/dev/null \
    || { cat "$WORK/register.log" 2>/dev/null; fail "registration of $CALLEE failed"; }
for _ in $(seq 1 30); do
    api GET /api/v1/telephony/status | jq -e --arg u "$CALLEE" '.registrations[] | select(.user == $u)' >/dev/null 2>&1 && break
    sleep 1
done
api GET /api/v1/telephony/status | jq -e --arg u "$CALLEE" '.registrations[] | select(.user == $u)' >/dev/null \
    || fail "registration of $CALLEE not visible in /api/v1/telephony/status"

log "caller calls extension 21"
printf 'SEQUENTIAL\n21;\n' > "$WORK/dest.csv"
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call.xml" -inf "$WORK/dest.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5093 -min_rtp_port 16100 -max_rtp_port 16150 -i "$SIP_HOST" -timeout 30 -timeout_error -trace_err -error_file "$WORK/caller.log" >/dev/null \
    || { cat "$WORK/caller.log" 2>/dev/null; fail "call failed"; }
wait "$CALLEE_PID" || { cat "$WORK/callee.log" 2>/dev/null; fail "callee scenario failed"; }

log "call record"
for _ in $(seq 1 15); do
    api GET /api/v1/calls | jq -e '.[] | select(.direction == "internal" and .destination == "21" and .billsec >= 1 and .billsec <= 10)' >/dev/null 2>&1 && break
    sleep 1
done
api GET /api/v1/calls | jq -e '.[] | select(.direction == "internal" and .destination == "21" and .billsec >= 1 and .billsec <= 10)' \
    || fail "no CDR for the call"

log "all end-to-end checks passed"
