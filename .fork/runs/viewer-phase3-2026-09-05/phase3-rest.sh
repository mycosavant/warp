#!/bin/bash
# Viewer phase 3: the record of a conversation, served to a paired device.
# Product profile + the wide bind (`warpdev.ps1 -Console`), agent in `auto`.
# Pairs the way a phone does -- the code from the QR's fragment, POSTed to
# /v1/pair, a credential per action from /v1/pair/credential -- and asks for
# the trace twice: whole, then the tail after the first reply's cursors.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
WEXE='C:\dev\warp\target\debug\warp-oss.exe'
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; mkdir -p "$OUT"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$OUT/driver.log"; }

log "instance already up; continuing"
log "cd the pane"
CTL input submit 'cd /home/effatha/git/warp' >> "$OUT/warpctrl.txt"; echo >> "$OUT/warpctrl.txt"
sleep 4

log "pairing"
CTL pair show | tee "$OUT/pair-show.json" >/dev/null
URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show.json'))['url'])")
ORIGIN=${URL#http://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}
log "origin $ORIGIN, code ${CODE:0:6}…"
/mnt/c/Windows/System32/curl.exe -s -m 5 -X POST -H "authorization: Bearer $CODE" "http://$ORIGIN/v1/pair" > "$OUT/pair.json"
python3 -c "import json;d=json.load(open('$OUT/pair.json'));print('actions:',' '.join(d['actions']))" | tee -a "$OUT/driver.log"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair.json'))['device_token'])")
cred() { /mnt/c/Windows/System32/curl.exe -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(cat /proc/sys/kernel/random/uuid)\",\"action\":\"$1\"}" "http://$ORIGIN/v1/pair/credential" \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])"; }
act() { local tok; tok=$(cred "$1"); /mnt/c/Windows/System32/curl.exe -s -m 20 -X POST -H "authorization: Bearer $tok" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(cat /proc/sys/kernel/random/uuid)\",\"action\":{\"kind\":\"$1\",\"params\":$2}}" "http://$ORIGIN/v1/control"; }

log "prompt"
PROMPT='Use your Read tool to read the first line of CLAUDE.md and quote it exactly. Then run `git log --oneline -1` with your Bash tool and quote its output. Two tool calls, then answer in two lines.'
CTL agent prompt "$PROMPT" | tee -a "$OUT/warpctrl.txt"
CID=$(grep -o '"conversation_id": *"[^"]*"' "$OUT/warpctrl.txt" | tail -1 | sed 's/.*: *"//;s/"//')
log "conversation $CID"
sleep 5
log "trace #1 from the paired device, mid-turn"
act agent.trace "{\"conversation_id\":\"$CID\"}" > "$OUT/trace-1.json"
python3 - "$OUT/trace-1.json" <<'PY' | tee -a "$OUT/driver.log"
import sys,json
d=json.load(open(sys.argv[1]))
r=d.get('response',{}); data=r.get('data',{})
if r.get('status')=='error': print('ERROR', r.get('error')); sys.exit()
h=data['header']; print('header:', {k:h.get(k) for k in ['warp_lines','harness','harness_file','harness_lines','joined_calls','clock_offset_ms','note']})
print('rows:', len(data['rows']), [ (x['from'],x['kind']) for x in data['rows']][:12])
PY
for i in $(seq 1 60); do
  sleep 3
  BUSY=$(CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d.get('conversations',[]) if c['conversation_id']=='$CID']; print(c[0]['is_busy'] if c else 'none')")
  [ "$BUSY" = "False" ] && break
done
log "turn over after ~$((i*3))s, is_busy=$BUSY"
WA=$(python3 -c "import json;print(json.load(open('$OUT/trace-1.json'))['response']['data']['header'].get('warp_lines',0))")
HA=$(python3 -c "import json;print(json.load(open('$OUT/trace-1.json'))['response']['data']['header'].get('harness_lines',0))")
log "trace #2, the tail after warp $WA / harness $HA"
act agent.trace "{\"conversation_id\":\"$CID\",\"warp_after\":$WA,\"harness_after\":$HA}" > "$OUT/trace-2.json"
python3 - "$OUT/trace-2.json" <<'PY' | tee -a "$OUT/driver.log"
import sys,json
d=json.load(open(sys.argv[1]))
r=d.get('response',{}); data=r.get('data',{})
if r.get('status')=='error': print('ERROR', r.get('error')); sys.exit()
h=data['header']; print('header:', {k:h.get(k) for k in ['warp_after','harness_after','warp_lines','harness_lines','joined_calls','note']})
print('rows:', len(data['rows']), [ (x['from'],x['kind']) for x in data['rows']])
PY
log "trace --live from the CLI (text)"
"$EXE" --warpctrl agent trace "$CID" --live > "$OUT/trace-live.txt" 2>&1; head -30 "$OUT/trace-live.txt" | tee -a "$OUT/driver.log"
log "the console in Brave, on the conversation"
echo "$URL" > "$OUT/console-url.txt"
