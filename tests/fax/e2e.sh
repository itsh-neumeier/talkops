#!/usr/bin/env bash
# Real fax media through the pinned FreeSWITCH image. Run only on the test stack.
set -euo pipefail
BASE=${BASE:-http://127.0.0.1:8080}
SIP_HOST=${SIP_HOST:-$(hostname -I | awk '{print $1}')}
SIP_TRUNK_PORT=${SIP_TRUNK_PORT:-5080}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
csrf=''
api() {
  local args=(-fsS -b "$work/cookies" -c "$work/cookies" -H 'X-Requested-With: TalkOps' -H "X-CSRF-Token: $csrf" -X "$1")
  [ $# -lt 3 ] || args+=(-H 'Content-Type: application/json' --data "$3")
  curl "${args[@]}" "$BASE/api/v1$2"
}
if api GET /setup | jq -e '.needs_setup' >/dev/null; then
  csrf=$(api POST /setup '{"username":"admin","display_name":"Admin","password":"e2e-password-1"}' | jq -er .csrf_token)
else
  csrf=$(api POST /auth/login '{"username":"admin","password":"e2e-password-1"}' | jq -er .csrf_token)
fi
owner=$(api GET /auth/me | jq -er .user.id)
trunk=$(api POST /trunks "$(jq -nc --arg host "$SIP_HOST:$SIP_TRUNK_PORT" '{name:"Fax media fixture",preset:"generic",enabled:true,overrides:{registrar:$host,proxy:$host,register:false}}')" | jq -er .id)
account=$(api POST "/trunks/$trunk/accounts" '{"username":"fax-e2e","password":"fax-e2e-secret","enabled":true}' | jq -er .id)
endpoint=$(python3 -c 'import uuid; print(uuid.uuid4())')
api PUT /fax-endpoints "$(jq -nc --arg id "$endpoint" --arg owner "$owner" '{id:$id,name:"Fax fixture inbox",owner_id:$owner,send_users:[],receive_users:[],reception_email:"",enabled:true}')" >/dev/null
source=$(api POST /numbers "$(jq -nc --arg trunk "$trunk" --arg account "$account" '{trunk_id:$trunk,account_id:$account,e164:"+49155510001",label:"Fax source",destination_type:"none",enabled:true}')" | jq -er .id)
api POST /numbers "$(jq -nc --arg trunk "$trunk" --arg account "$account" --arg endpoint "$endpoint" '{trunk_id:$trunk,account_id:$account,e164:"+49155510002",label:"Fax receiver",destination_type:"fax",destination_id:$endpoint,enabled:true}')" >/dev/null
config=$(api GET /settings/fax | jq -c --arg source "$source" '.config | .enabled=true | .global_number_id=$source')
payload=$(docker compose exec -T media-worker python3 - <<'PY'
import base64,io,json
from PIL import Image,ImageDraw
from reportlab.pdfgen import canvas
image=Image.new('RGB',(900,1200),'white')
ImageDraw.Draw(image).text((50,100),'TalkOps image attachment',fill='black')
png=io.BytesIO(); image.save(png,format='PNG')
pdf=io.BytesIO(); c=canvas.Canvas(pdf); c.drawString(40,700,'TalkOps PDF attachment'); c.showPage(); c.save()
print(json.dumps({'destination':'+49155510002','files':[{'name':'one.pdf','data':base64.b64encode(pdf.getvalue()).decode()},{'name':'two.png','data':base64.b64encode(png.getvalue()).decode()}]}))
PY
)
payload=$(echo "$payload" | jq -c --arg source "$source" '.number_id=$source')
for mode in audio required; do
  printf 'Fax media test starting: %s\n' "$mode"
  previous=$(api GET /faxes | jq -c '[.[] | select(.direction=="inbound") | .id]')
  config=$(echo "$config" | jq -c --arg trunk "$trunk" --arg mode "$mode" '.trunk_modes[$trunk]=$mode')
  api PUT /settings/fax "$(jq -nc --argjson config "$config" '{config:$config}')" >/dev/null
  id=$(api POST /faxes "$payload" | jq -er .id)
  ready=false
  for _ in $(seq 1 60); do
    state=$(api GET "/faxes/$id/report" | jq -r .state)
    if [ "$state" = draft ]; then ready=true; break; fi
    [ "$state" != failed ] || { api GET "/faxes/$id/report"; exit 1; }
    sleep 2
  done
  [ "$ready" = true ] || { echo 'Fax conversion did not finish'; exit 1; }
  api POST "/faxes/$id/submit" >/dev/null
  completed=false
  for _ in $(seq 1 120); do
    report=$(api GET "/faxes/$id/report")
    state=$(echo "$report" | jq -r .state)
    if [ "$state" = succeeded ]; then completed=true; break; fi
    if [ "$state" = failed ] || [ "$state" = unknown ]; then echo "$report"; exit 1; fi
    sleep 2
  done
  if [ "$completed" != true ]; then
    echo 'Fax media transfer did not finish'
    echo "$report"
    api GET /faxes
    docker compose exec -T freeswitch sh -c 'fs_cli -p "$TALKOPS_ESL_PASSWORD" -x "show channels as json"'
    for call in $(api GET /faxes | jq -r '.[].call_uuid // empty'); do
      docker compose exec -T freeswitch sh -c 'for var in rtp_use_timer_name local_media_ip local_media_port remote_media_ip remote_media_port fax_t38_status; do printf "%s %s: " "$1" "$var"; fs_cli -p "$TALKOPS_ESL_PASSWORD" -x "uuid_getvar $1 $var"; done' sh "$call"
    done
    exit 1
  fi
  echo "$report" | jq -e '.pages == 2 and .attempts == 1' >/dev/null
  printf 'Fax transfer confirmed; checking received PDF: %s\n' "$mode"
  if [ "$mode" = required ]; then
    echo "$report" | jq -e '.result | endswith("transport negotiated")' >/dev/null
  else
    echo "$report" | jq -e '.result | endswith("transport off")' >/dev/null
  fi
  for _ in $(seq 1 30); do
    inbox=$(api GET /faxes)
    if echo "$inbox" | jq -e --arg endpoint "$endpoint" --argjson previous "$previous" 'any(.[]; . as $fax | .direction=="inbound" and .endpoint_id==$endpoint and .state=="succeeded" and .pages==2 and ($previous | index($fax.id) | not))' >/dev/null; then break; fi
    sleep 2
  done
  received=$(echo "$inbox" | jq -er --arg endpoint "$endpoint" --argjson previous "$previous" '[.[] | select(. as $fax | .direction=="inbound" and .endpoint_id==$endpoint and .state=="succeeded" and .pages==2 and ($previous | index($fax.id) | not))][0].id')
  api GET "/faxes/$received/document" > "$work/received.pdf"
  grep -aq '%PDF-' "$work/received.pdf"
  printf 'Fax media round trip passed: %s\n' "$mode"
done
