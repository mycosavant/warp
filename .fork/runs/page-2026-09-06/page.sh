#!/bin/bash
# Steps 5 and 6 on the Windows build, as far as a desktop browser can show
# them, plus the chip flipping back when a code dies. The rig profile so an
# Edit asks and the phone answers it; Brave in a scratch profile with
# certificate errors ignored stands in for a phone that installed the
# authority. What a desktop cannot show: a notification arriving on a phone,
# which needs the permission prompt tapped by a person and a phone.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
BRAVE='C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe'
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
uuid() { cat /proc/sys/kernel/random/uuid; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process "${2:-warp-oss}" -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }
say() { python3 -c "import sys,json;d=json.load(sys.stdin);r=d.get('response',d);e=r.get('error') or d.get('error');print('  ', 'REFUSED' if e else 'OK', (e or {}).get('code',''), '-', (e or {}).get('message','') or json.dumps(r.get('data',''))[:100])"; }
TLS_CURL() { "$CURL" --ssl-no-revoke --cacert "$OUT/ca.crt" "$@"; }
approvals_of() { python3 -c "import json;d=json.load(open('$1'));r=d.get('response',d);a=(r.get('data') or {}).get('approvals') or r.get('approvals') or [];print(json.dumps(a))"; }

log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version) ($(ls -la --time-style=+%F\ %H:%M /mnt/c/dev/warp/target/release/warp-oss.exe | awk '{print $6, $7}'))"
log "launch: warpdev.ps1 -Instrumented -Console -Bind $BIND"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 3
"$CURL" -s -m 5 -o "$OUT/ca.crt" "http://$BIND/ca.crt"
CTL tab create > "$OUT/tab.json"; sleep 2
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
STAMP=$(date +%s); F=/tmp/page-$STAMP.txt; printf 'alpha\nbeta\ngamma\n' > "$F"
CTL agent prompt "Using your Edit tool, change the line beta to delta in $F. Say done." > "$OUT/prompt-c.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-c.json'))['conversation_id'])"); echo "$C" > "$OUT/conversation-c.txt"
log "conversation C $C; the chip once, then the phone pairs and answers the Edit"
sleep 3; CLICK 905 516 | tee -a "$LOG"; sleep 3
URL=$(powershell.exe -NoProfile -Command "Get-Clipboard" | tr -d '\r\n'); ORIGIN=${URL#https://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}
TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $CODE" "https://$ORIGIN/v1/pair" > "$OUT/pair.json"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair.json'))['device_token'])")
cred() { TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "https://$ORIGIN/v1/pair/credential" | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])"; }
control() { TLS_CURL -s -m 20 -X POST -H "authorization: Bearer $1" -H 'content-type: application/json' -H "origin: https://$ORIGIN" -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"$2\",\"params\":$3}}" "https://$ORIGIN/v1/control"; }
ATOK=$(cred agent.approvals)
for i in $(seq 1 40); do sleep 3; control "$ATOK" agent.approvals '{}' > "$OUT/approvals.json"; [ "$(approvals_of "$OUT/approvals.json")" != "[]" ] && break; done
AID=$(approvals_of "$OUT/approvals.json" | python3 -c "import sys,json;a=json.load(sys.stdin);print(a[0]['approval_id'] if a else '')")
DIG=$(approvals_of "$OUT/approvals.json" | python3 -c "import sys,json;a=json.load(sys.stdin);print(a[0]['digest'] if a else '')")
if [ -n "$AID" ]; then control "$(cred agent.approve)" agent.approve "{\"approval_id\":\"$AID\",\"digest\":\"$DIG\"}" | tee "$OUT/approve.json" | say | tee -a "$LOG"; else log "  no ask in 120 s"; fi
wait_idle "$C"; sleep 2; log "file now: $(tr '\n' ' ' < "$F")"

log "--- Brave, certificate errors ignored, opens a second control code for C"
CTL pair show --conversation "$C" > "$OUT/pair-show-brave.json"
BURL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show-brave.json'))['url'])")
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null; sleep 1
powershell.exe -NoProfile -Command "Start-Process -FilePath '$BRAVE' -ArgumentList '--user-data-dir=C:\dev\brave-scratch-page','--no-first-run','--ignore-certificate-errors','--window-size=430,1000','--window-position=40,40','$BURL'" 2>&1 | tail -1
sleep 8; SHOT page-conversation brave
log "  (header with mode/model/agent/cwd, the notify button, the Edit as a diff: page-conversation.png)"
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null

log "--- the chip flips back on its own: a code left to die"
CTL agent prompt 'Say the single word second and nothing else.' > "$OUT/prompt-d.json"
D=$(python3 -c "import json;print(json.load(open('$OUT/prompt-d.json'))['conversation_id'])"); wait_idle "$D"; sleep 2
CLICK 905 516 | tee -a "$LOG"; sleep 3; SHOT chip-waiting
log "waiting 125 s"; sleep 125; SHOT chip-after-expiry
log "  (chip should read /remote-control, block should read expired: chip-after-expiry.png)"

log "--- close"
CTL window close > "$OUT/close.json"; sleep 8
N=$(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])))")
if [ "$N" != "0" ]; then log "close asked to confirm; asking again"; CTL window close >/dev/null; sleep 8; N=$(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])))"); fi
log "after close: $N record(s)"; log "done"
