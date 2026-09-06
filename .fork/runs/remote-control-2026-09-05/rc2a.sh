#!/bin/bash
# Second run, on the build with the two fixes: a revoked device is refused at
# once, and a text-only turn joins the harness's file. Instance already up.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver2.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
uuid() { cat /proc/sys/kernel/random/uuid; }
wait_idle() { for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; }

log "cd the pane, one text-only conversation"
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-c.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-c.json'))['conversation_id'])")
log "conversation C $C"; wait_idle "$C"; echo "$C" > "$OUT/conversation-c.txt"

log "trace C as the machine: does a text-only turn join now?"
"$EXE" --warpctrl agent trace "$C" --live --output-format json > "$OUT/trace-c.json" 2>&1
python3 -c "
import json;d=json.load(open('$OUT/trace-c.json'));h=d['header']
print('  linked_session_id:',h.get('linked_session_id'));print('  harness_file:',h.get('harness_file'));print('  harness_lines:',h.get('harness_lines'),'note:',h.get('note'))
print('  agent text rows:',[ (r.get('text') or '')[:40] for r in d['rows'] if r['from']=='harness' and r['kind']=='text'])" | tee -a "$LOG"

log "pair a device for C"
CTL pair show --conversation "$C" > "$OUT/pair-show-2.json"
URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show-2.json'))['url'])")
ORIGIN=${URL#http://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}
"$CURL" -s -m 5 -X POST -H "authorization: Bearer $CODE" "http://$ORIGIN/v1/pair" > "$OUT/pair-2.json"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair-2.json'))['device_token'])")
cred() { "$CURL" -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "http://$ORIGIN/v1/pair/credential"; }
call() { "$CURL" -s -m 20 -X POST -H "authorization: Bearer $1" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"agent.prompt\",\"params\":{\"prompt\":\"$2\",\"conversation_id\":\"$C\"}}}" "http://$ORIGIN/v1/control"; }
TOK=$(cred agent.prompt | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])")
log "prompt C on the credential, before the stop"
call "$TOK" "Reply with the single word before and nothing else." | python3 -c "import sys,json;d=json.load(sys.stdin);r=d.get('response',d);print('  ',r.get('status'),json.dumps(r.get('data',r.get('error')))[:120])" | tee -a "$LOG"
wait_idle "$C"

