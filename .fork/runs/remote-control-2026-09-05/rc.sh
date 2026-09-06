#!/bin/bash
# Remote control, measured: a code minted for one conversation, a device that
# spends it, and what that device may and may not do. Instance already up
# (`warpdev.ps1 -Console`), driven from WSL with `curl.exe` for the phone's
# side (the Windows stack is where the wide listener is).
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; mkdir -p "$OUT"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$OUT/driver.log"; }
uuid() { cat /proc/sys/kernel/random/uuid; }

log "cd the pane, two conversations"
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-a.json"
A=$(python3 -c "import json;print(json.load(open('$OUT/prompt-a.json'))['conversation_id'])")
log "conversation A $A"
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$A']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done
CTL agent prompt 'Say the single word second and nothing else.' > "$OUT/prompt-b.json"
B=$(python3 -c "import json;print(json.load(open('$OUT/prompt-b.json'))['conversation_id'])")
log "conversation B $B (the one handed over)"
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$B']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done

log "pair show --conversation B"
CTL pair show --conversation "$B" > "$OUT/pair-show.json"
python3 -c "import json;d=json.load(open('$OUT/pair-show.json'));print('actions:',' '.join(d['actions']));print('confined:',d.get('conversation_id'))" | tee -a "$OUT/driver.log"
URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show.json'))['url'])")
ORIGIN=${URL#http://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}
"$CURL" -s -m 5 -X POST -H "authorization: Bearer $CODE" "http://$ORIGIN/v1/pair" > "$OUT/pair.json"
python3 -c "import json;d=json.load(open('$OUT/pair.json'));print('device actions:',' '.join(d['actions']));print('device confined:',d.get('conversation_id'))" | tee -a "$OUT/driver.log"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair.json'))['device_token'])")
cred() { "$CURL" -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "http://$ORIGIN/v1/pair/credential"; }
act() { local c tok; c=$(cred "$1"); tok=$(echo "$c" | python3 -c "import sys,json;d=json.load(sys.stdin);print(d.get('bearer_token',''))")
  if [ -z "$tok" ]; then echo "$c"; return; fi
  "$CURL" -s -m 20 -X POST -H "authorization: Bearer $tok" -H 'content-type: application/json' \
  -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"$1\",\"params\":$2}}" "http://$ORIGIN/v1/control"; }
say() { python3 -c "
import sys,json
d=json.load(sys.stdin); r=d.get('response',d)
if r.get('status')=='error' or 'error' in d and 'response' not in d:
    e=(r.get('error') or d.get('error')); print('REFUSED:', e.get('code'), '-', e.get('message'))
else:
    print('OK:', json.dumps(r.get('data',r))[:200])"; }

log "credential for agent.prompt, what the grant says"
cred agent.prompt | tee "$OUT/cred-prompt.json" | python3 -c "import sys,json;d=json.load(sys.stdin);print('grant:',d.get('grant',{}).get('action'),'conversation',d.get('grant',{}).get('conversation'))" | tee -a "$OUT/driver.log"
log "agent.list as the device (filtered?)"
act agent.list '{}' | tee "$OUT/list.json" | python3 -c "import sys,json;d=json.load(sys.stdin);print('sees:',[c['conversation_id'] for c in d['response']['data']['conversations']])" | tee -a "$OUT/driver.log"
log "prompt A from the device (must be refused)"
act agent.prompt "{\"prompt\":\"hijack\",\"conversation_id\":\"$A\"}" | tee "$OUT/prompt-other.json" | say | tee -a "$OUT/driver.log"
log "prompt with no conversation (must be refused)"
act agent.prompt '{"prompt":"new one"}' | tee "$OUT/prompt-new.json" | say | tee -a "$OUT/driver.log"
log "trace A (must be refused)"
act agent.trace "{\"conversation_id\":\"$A\"}" | tee "$OUT/trace-other.json" | say | tee -a "$OUT/driver.log"
log "prompt B from the device (must run)"
act agent.prompt "{\"prompt\":\"Reply with the single word phone and nothing else.\",\"conversation_id\":\"$B\"}" | tee "$OUT/prompt-mine.json" | say | tee -a "$OUT/driver.log"
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$B']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done
log "B's record as the device, last rows"
act agent.trace "{\"conversation_id\":\"$B\"}" > "$OUT/trace-mine.json"
python3 -c "
import json;d=json.load(open('$OUT/trace-mine.json'))['response']['data']
for r in d['rows'][-6:]: print(' ',r['from'],r['kind'],(r.get('text') or '')[:60].replace(chr(10),' '))" | tee -a "$OUT/driver.log"
CTL agent read "$B" > "$OUT/read-b.json"
log "the pane the prompt landed in: agent read B says $(python3 -c "import json;d=json.load(open('$OUT/read-b.json'));print(d['conversation'].get('pane_id'), 'exchanges', d['exchange_count'])")"
log "stop sharing from the CLI side: no verb for it, so via the chip later; a second pair show for A stays watch-scoped:"
CTL pair show | python3 -c "import sys,json;d=json.load(sys.stdin);print('  watch code actions:',' '.join(d['actions']),'confined:',d.get('conversation_id'))" | tee -a "$OUT/driver.log"
log "agent trace --live from the CLI"
"$EXE" --warpctrl agent trace "$B" --live 2>&1 | head -12 | tee "$OUT/trace-live.txt" | tee -a "$OUT/driver.log"
echo "$B" > "$OUT/conversation-b.txt"
