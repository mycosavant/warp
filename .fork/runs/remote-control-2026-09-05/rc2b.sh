#!/bin/bash
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe; CURL=/mnt/c/Windows/System32/curl.exe
OUT=${1:?outdir}; LOG="$OUT/driver2.log"; X=${2:-890}; Y=${3:-516}
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
uuid() { cat /proc/sys/kernel/random/uuid; }
C=$(cat "$OUT/conversation-c.txt"); URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show-2.json'))['url'])")
ORIGIN=${URL#http://}; ORIGIN=${ORIGIN%%/*}
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair-2.json'))['device_token'])")
cred() { "$CURL" -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "http://$ORIGIN/v1/pair/credential"; }
call() { "$CURL" -s -m 20 -X POST -H "authorization: Bearer $1" -H 'content-type: application/json' -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"agent.prompt\",\"params\":{\"prompt\":\"$2\",\"conversation_id\":\"$C\"}}}" "http://$ORIGIN/v1/control"; }
say() { python3 -c "import sys,json;d=json.load(sys.stdin);r=d.get('response',d);e=r.get('error') or d.get('error');print('  ', 'REFUSED' if e else 'OK', (e or {}).get('code',''), '-', (e or {}).get('message','') or json.dumps(r.get('data',''))[:80])"; }
log "a prompt credential minted before the stop, kept"
TOK=$(cred agent.prompt | tee "$OUT/cred-before-stop.json" | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])")
log "Stop sharing: the chip at ($X,$Y), clicked"
powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$X" -Y "$Y" 2>&1 | tail -1 | tee -a "$LOG"
sleep 3
powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\rc2-after-stop.png' 2>&1 | tail -1; cp /mnt/c/dev/shots/rc2-after-stop.png "$OUT/rc2-after-stop.png"
log "the kept credential, after the stop (must be refused)"
call "$TOK" "Reply with the single word after and nothing else." | tee "$OUT/prompt-after-stop.json" | say | tee -a "$LOG"
log "a fresh credential from the revoked device (must be refused)"
cred agent.prompt | tee "$OUT/cred-after-stop.json" | say | tee -a "$LOG"
sleep 3
"$EXE" --warpctrl agent read "$C" --output-format json > "$OUT/read-c.json"
log "C has $(python3 -c "import json;print(json.load(open('$OUT/read-c.json'))['exchange_count'])") exchanges (2 expected: ready, before; no 'after')"
