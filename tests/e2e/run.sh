#!/usr/bin/env bash
# End-to-end test against a running TalkOps + FreeSWITCH stack.
#
# Creates an admin, two extensions with one device each, registers device
# 21-1 with SIPp, calls it from 20-1 and checks the call record. Also checks
# that a wrong SIP password is rejected, that DND rejects calls as busy and
# that a provisioned Yealink phone gets its configuration and that an
# unreachable extension's voicemail answers and stores the message, that a
# ring group falls back, an IVR menu routes a pressed key and a queue
# offers its caller to a registered agent, that answered calls are
# recorded, that a door station's ring reaches a phone with video and
# (WEBRTC_E2E=1, needs Node.js with Playwright) that the browser softphone
# can call an extension.
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

log "do not disturb rejects calls with 486"
EXT21=$(api GET /api/v1/extensions | jq -r '.[] | select(.number == "21") | .id')
api PUT "/api/v1/extensions/$EXT21/call-settings" '{"dnd":true}' >/dev/null || fail "enable DND"
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_busy.xml" -inf "$WORK/dest.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5095 -min_rtp_port 16200 -max_rtp_port 16250 -i "$SIP_HOST" -timeout 30 -timeout_error -trace_err -error_file "$WORK/busy.log" >/dev/null \
    || { cat "$WORK/busy.log" 2>/dev/null; fail "DND call was not rejected as busy"; }
api PUT "/api/v1/extensions/$EXT21/call-settings" '{"dnd":false}' >/dev/null

log "Yealink provisioning"
PHONE=$(api POST /api/v1/phones '{"mac":"80:5e:c0:00:e2:e2","model":"t54w","name":"E2E","line_keys":[{"key":3,"type":"blf","value":"21","label":"E2E 21"}]}' | jq -r .id) \
    || fail "create phone"
api POST "/api/v1/extensions/$EXT21/devices" "{\"name\":\"Yealink\",\"kind\":\"desk\",\"phone_id\":\"$PHONE\"}" >/dev/null || fail "place device on phone"
PROV=$(api GET /api/v1/provisioning)
PUSER=$(echo "$PROV" | jq -r .username)
PPASS=$(echo "$PROV" | jq -r .password)
curl -sf -u "$PUSER:$PPASS" "$BASE/provisioning/y000000000068.cfg" | grep -q '^auto_provision.server.url = ' \
    || fail "common configuration"
CFG=$(curl -sf -u "$PUSER:$PPASS" -A "Yealink SIP-T54W 96.86.0.100 80:5e:c0:00:e2:e2" "$BASE/provisioning/805ec000e2e2.cfg") \
    || fail "phone configuration"
echo "$CFG" | grep -q '^account.1.user_name = 21-2$' || fail "account missing in phone configuration"
echo "$CFG" | grep -q '^linekey.3.type = 16$' || fail "BLF key missing in phone configuration"
[ "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/provisioning/805ec000e2e2.cfg")" = 401 ] \
    || fail "provisioning without credentials must be rejected"
curl -sf -u "$PUSER:$PPASS" "$BASE/provisioning/phonebook/internal.xml" | grep -q '<Name>E2E 21</Name>' \
    || fail "internal phonebook"

log "voicemail answers and stores a message"
EXT22=$(api POST /api/v1/extensions '{"number":"22","display_name":"E2E 22"}' | jq -r .id) || fail "create extension 22"
api PUT "/api/v1/extensions/$EXT22/voicemail" '{"enabled":true,"pin":"2468"}' >/dev/null || fail "enable voicemail"
printf 'SEQUENTIAL\n22;\n' > "$WORK/vm.csv"
(cd "$DIR" && sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_voicemail.xml" -inf "$WORK/vm.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5096 -min_rtp_port 16300 -max_rtp_port 16350 -i "$SIP_HOST" -timeout 60 -timeout_error -trace_err -error_file "$WORK/vm.log" >/dev/null) \
    || { cat "$WORK/vm.log" 2>/dev/null; fail "voicemail call failed"; }
for _ in $(seq 1 15); do
    api GET "/api/v1/voicemail/messages?extension_id=$EXT22" | jq -e 'length >= 1 and .[0].duration_secs >= 1' >/dev/null 2>&1 && break
    sleep 1
done
api GET "/api/v1/voicemail/messages?extension_id=$EXT22" | jq -e 'length >= 1 and .[0].duration_secs >= 1' \
    || fail "no voicemail message stored"
MSG=$(api GET "/api/v1/voicemail/messages?extension_id=$EXT22" | jq -r '.[0].id')
curl -sf -b "$COOKIES" -o "$WORK/vm.wav" "$BASE/api/v1/voicemail/messages/$MSG/audio" \
    || fail "voicemail recording cannot be downloaded"
[ "$(head -c 4 "$WORK/vm.wav")" = RIFF ] || fail "voicemail recording is not a WAV file"

vm_count() {
    api GET "/api/v1/voicemail/messages?extension_id=$EXT22" | jq length
}
wait_vm_count() { # expected
    for _ in $(seq 1 15); do
        [ "$(vm_count)" -ge "$1" ] && return 0
        sleep 1
    done
    return 1
}

log "ring group without reachable members falls back to voicemail"
EXT21=$(api GET /api/v1/extensions | jq -r '.[] | select(.number == "21") | .id')
api POST /api/v1/ring-groups "{\"number\":\"50\",\"name\":\"E2E group\",\"members\":[\"$EXT21\"],\"ring_timeout_secs\":5,\"fallback_type\":\"voicemail\",\"fallback_id\":\"$EXT22\"}" >/dev/null \
    || fail "create ring group"
printf 'SEQUENTIAL\n50;\n' > "$WORK/group.csv"
(cd "$DIR" && sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_voicemail.xml" -inf "$WORK/group.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5097 -min_rtp_port 16400 -max_rtp_port 16450 -i "$SIP_HOST" -timeout 60 -timeout_error -trace_err -error_file "$WORK/group.log" >/dev/null) \
    || { cat "$WORK/group.log" 2>/dev/null; fail "ring group call failed"; }
wait_vm_count 2 || fail "ring group fallback did not reach voicemail"

log "IVR menu routes key 1"
api POST /api/v1/ivr-menus "{\"number\":\"70\",\"name\":\"E2E menu\",\"greeting_text\":\"Drücken Sie die 1.\",\"timeout_secs\":4,\"options\":[{\"digit\":\"1\",\"type\":\"voicemail\",\"id\":\"$EXT22\"}]}" >/dev/null \
    || fail "create IVR menu"
printf 'SEQUENTIAL\n70;\n' > "$WORK/ivr.csv"
(cd "$DIR" && sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_ivr.xml" -inf "$WORK/ivr.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5098 -min_rtp_port 16500 -max_rtp_port 16550 -i "$SIP_HOST" -timeout 60 -timeout_error -trace_err -error_file "$WORK/ivr.log" >/dev/null) \
    || { cat "$WORK/ivr.log" 2>/dev/null; fail "IVR call failed"; }
wait_vm_count 3 || fail "IVR choice did not reach voicemail"

log "queue offers the call to an agent"
api POST /api/v1/queues "{\"number\":\"80\",\"name\":\"E2E queue\",\"strategy\":\"ring-all\",\"members\":[\"$EXT21\"]}" >/dev/null \
    || fail "create queue"
sipp -sn uas -p 5094 -i "$SIP_HOST" -m 1 -min_rtp_port 16600 -max_rtp_port 16650 -timeout 60 -timeout_error \
    -trace_err -error_file "$WORK/agent.log" >"$WORK/agent.out" 2>&1 &
AGENT_PID=$!
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/register.xml" -set contact_port 5094 -s "$CALLEE" -au "$CALLEE" -ap "$CALLEE_PW" \
    -m 1 -p 5092 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/register2.log" >/dev/null \
    || { cat "$WORK/register2.log" 2>/dev/null; fail "agent registration failed"; }
# The queue sync adds the agent within seconds of the change.
sleep 3
printf 'SEQUENTIAL\n80;\n' > "$WORK/queue.csv"
(cd "$DIR" && sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_hold.xml" -inf "$WORK/queue.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5099 -min_rtp_port 16700 -max_rtp_port 16750 -i "$SIP_HOST" -timeout 60 -timeout_error -trace_err -error_file "$WORK/queue.log" >/dev/null) \
    || { cat "$WORK/queue.log" 2>/dev/null; fail "queue call failed"; }
wait "$AGENT_PID" || { cat "$WORK/agent.log" 2>/dev/null; fail "agent did not receive the queue call"; }

log "answered calls are recorded"
SETTINGS=$(api GET /api/v1/settings | jq '.record_internal = true') || fail "read settings"
api PUT /api/v1/settings "$SETTINGS" >/dev/null || fail "enable recording"
sipp -sn uas -p 5094 -i "$SIP_HOST" -m 1 -min_rtp_port 16800 -max_rtp_port 16850 -timeout 60 -timeout_error \
    -trace_err -error_file "$WORK/rec-callee.log" >"$WORK/rec-callee.out" 2>&1 &
REC_PID=$!
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/register.xml" -set contact_port 5094 -s "$CALLEE" -au "$CALLEE" -ap "$CALLEE_PW" \
    -m 1 -p 5092 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/register3.log" >/dev/null \
    || { cat "$WORK/register3.log" 2>/dev/null; fail "registration for the recorded call failed"; }
(cd "$DIR" && sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_hold.xml" -inf "$WORK/dest.csv" -s "$CALLER" -au "$CALLER" -ap "$CALLER_PW" \
    -m 1 -p 5097 -min_rtp_port 16900 -max_rtp_port 16950 -i "$SIP_HOST" -timeout 60 -timeout_error -trace_err -error_file "$WORK/rec.log" >/dev/null) \
    || { cat "$WORK/rec.log" 2>/dev/null; fail "recorded call failed"; }
wait "$REC_PID" || { cat "$WORK/rec-callee.log" 2>/dev/null; fail "callee of the recorded call failed"; }
REC=""
for _ in $(seq 1 15); do
    REC=$(api GET /api/v1/calls | jq -r '[.[] | select(.destination == "21" and .recording_id != null)][0].recording_id // empty')
    [ -n "$REC" ] && break
    sleep 1
done
[ -n "$REC" ] || fail "no recording linked to the call"
api GET "/api/v1/recordings/$REC" | jq -e '.duration_secs >= 5' >/dev/null || fail "recording too short"
api GET "/api/v1/recordings/$REC/audio" > "$WORK/rec.wav" || fail "download recording"
[ "$(head -c 4 "$WORK/rec.wav")" = RIFF ] || fail "recording is not a WAV file"

log "door station rings with video"
read -r DOOR DOOR_PW <<<"$(device 8001)"
EXT8001=$(api GET /api/v1/extensions | jq -r '.[] | select(.number == "8001") | .id')
api POST /api/v1/door-stations "{\"name\":\"E2E door\",\"extension_id\":\"$EXT8001\",\"destination_type\":\"extension\",\"destination_id\":\"$EXT21\"}" >/dev/null \
    || fail "create door station"
sipp -sf "$DIR/uas_video.xml" -p 5094 -i "$SIP_HOST" -m 1 -min_rtp_port 17000 -max_rtp_port 17050 -timeout 60 -timeout_error \
    -trace_err -error_file "$WORK/door-callee.log" >"$WORK/door-callee.out" 2>&1 &
DOOR_PID=$!
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/register.xml" -set contact_port 5094 -s "$CALLEE" -au "$CALLEE" -ap "$CALLEE_PW" \
    -m 1 -p 5092 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/register4.log" >/dev/null \
    || { cat "$WORK/register4.log" 2>/dev/null; fail "registration for the door call failed"; }
printf 'SEQUENTIAL\n9901;\n' > "$WORK/door.csv"
sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/call_video.xml" -inf "$WORK/door.csv" -s "$DOOR" -au "$DOOR" -ap "$DOOR_PW" \
    -m 1 -p 5096 -min_rtp_port 17100 -max_rtp_port 17150 -i "$SIP_HOST" -timeout 30 -timeout_error -trace_err -error_file "$WORK/door.log" >/dev/null \
    || { cat "$WORK/door.log" 2>/dev/null; fail "door station call failed"; }
wait "$DOOR_PID" || { cat "$WORK/door-callee.log" 2>/dev/null; fail "the door call did not reach the phone with video"; }
api GET /api/v1/door-events | jq -e '.[] | select(.kind == "ring" and .detail.dialed == "9901")' >/dev/null \
    || fail "door ring not logged"

if [ "${WEBRTC_E2E:-0}" = 1 ]; then
    log "browser softphone (WebRTC) calls an extension"
    WEBU=$(api POST /api/v1/users '{"username":"e2e-web","display_name":"E2E Web","role":"user","password":"e2e-web-password"}' | jq -r .id) \
        || fail "create web user"
    api POST /api/v1/extensions "{\"number\":\"23\",\"display_name\":\"E2E 23\",\"user_id\":\"$WEBU\"}" >/dev/null \
        || fail "create extension 23"
    sipp -sn uas -p 5094 -i "$SIP_HOST" -m 1 -min_rtp_port 17200 -max_rtp_port 17250 -timeout 60 -timeout_error \
        -trace_err -error_file "$WORK/web-callee.log" >"$WORK/web-callee.out" 2>&1 &
    WEB_PID=$!
    sipp "$SIP_HOST:$SIP_PORT" -sf "$DIR/register.xml" -set contact_port 5094 -s "$CALLEE" -au "$CALLEE" -ap "$CALLEE_PW" \
        -m 1 -p 5092 -i "$SIP_HOST" -timeout 20 -timeout_error -trace_err -error_file "$WORK/register5.log" >/dev/null \
        || { cat "$WORK/register5.log" 2>/dev/null; fail "registration for the softphone call failed"; }
    node "$DIR/../e2e/webrtc.mjs" "$BASE" e2e-web e2e-web-password 21 || fail "softphone call"
    wait "$WEB_PID" || { cat "$WORK/web-callee.log" 2>/dev/null; fail "callee of the softphone call failed"; }
fi

log "all end-to-end checks passed"
