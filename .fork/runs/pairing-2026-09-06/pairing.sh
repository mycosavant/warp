#!/bin/bash
# Steps 3 and 4 of .fork/HANDOFF-MOBILE.md, measured on the Windows build:
# a control pairing with no clock (the redeem carries no expires_at), the
# block's three states (waiting, paired at, expired), and the record saying
# which prompt and which answer came from the phone (`via: paired_device` in
# the event log, "from the phone" in the trace). The rig profile
# (-Instrumented) puts the agent in `default` mode so a Write asks, which is
# what the phone answers. The phone is curl.exe over TLS with the authority.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
DATA=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
uuid() { cat /proc/sys/kernel/random/uuid; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }
say() { python3 -c "import sys,json;d=json.load(sys.stdin);r=d.get('response',d);e=r.get('error') or d.get('error');print('  ', 'REFUSED' if e else 'OK', (e or {}).get('code',''), '-', (e or {}).get('message','') or json.dumps(r.get('data',''))[:100])"; }
TLS_CURL() { "$CURL" --ssl-no-revoke --cacert "$OUT/ca.crt" "$@"; }

log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version) ($(ls -la --time-style=+%F\ %H:%M /mnt/c/dev/warp/target/release/warp-oss.exe | awk '{print $6, $7}'))"
log "instances before: $(CTL instance list | python3 -c "import sys,json;print(len(json.load(sys.stdin).get('instances',[])))")"
log "launch: warpdev.ps1 -Instrumented -Console -Bind $BIND (agent in default mode, so a Write asks)"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 3
log "instance: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
"$CURL" -s -m 5 -o "$OUT/ca.crt" "http://$BIND/ca.crt"; log "authority fetched in the clear: $(grep -c 'BEGIN CERTIFICATE' "$OUT/ca.crt") certificate; same as the tls run's: $(cmp -s "$OUT/ca.crt" "$OUT/../tls-2026-09-06/ca.crt" && echo yes || echo NO)"

log "--- a conversation, the chip once: waiting"
CTL tab create > "$OUT/tab.json"; sleep 2
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-c.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-c.json'))['conversation_id'])")
log "conversation C $C"; wait_idle "$C"; echo "$C" > "$OUT/conversation-c.txt"; sleep 2
SHOT pairing-footer
CLICK 905 516 | tee -a "$LOG"; sleep 3
SHOT state-waiting
URL=$(powershell.exe -NoProfile -Command "Get-Clipboard" | tr -d '\r\n'); log "clipboard: ${URL%%#*}#<code>"
ORIGIN=${URL#https://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}

log "--- the phone scans: redeem over TLS"
TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $CODE" "https://$ORIGIN/v1/pair" > "$OUT/pair.json"
log "redeem: $(python3 -c "import json;d=json.load(open('$OUT/pair.json'));print('device' if d.get('device_token') else d, '| expires_at:', d.get('expires_at','ABSENT (no clock)'), '| confined:', d.get('conversation_id'))")"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair.json'))['device_token'])")
sleep 4; SHOT state-paired
cred() { TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "https://$ORIGIN/v1/pair/credential" | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])"; }
control() { TLS_CURL -s -m 20 -X POST -H "authorization: Bearer $1" -H 'content-type: application/json' -H "origin: https://$ORIGIN" -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"$2\",\"params\":$3}}" "https://$ORIGIN/v1/control"; }

log "--- a prompt from the phone that makes the agent ask"
STAMP=$(date +%s)
control "$(cred agent.prompt)" agent.prompt "{\"prompt\":\"Create the file /tmp/phone-$STAMP.txt containing the single word hello. Use your Write tool. Say done when it exists.\",\"conversation_id\":\"$C\"}" | tee "$OUT/prompt-phone.json" | say | tee -a "$LOG"
APPROVALS_TOK=$(cred agent.approvals)
for i in $(seq 1 40); do
  sleep 3
  control "$APPROVALS_TOK" agent.approvals '{}' > "$OUT/approvals.json"
  python3 -c "import json;d=json.load(open('$OUT/approvals.json'));r=d.get('response',d);a=(r.get('data') or {}).get('approvals') or r.get('approvals') or [];import sys;sys.exit(0 if a else 1)" && break
done
python3 - "$OUT/approvals.json" <<'EOF' | tee -a "$LOG"
import json,sys
d=json.load(open(sys.argv[1])); r=d.get('response',d); a=(r.get('data') or {}).get('approvals') or r.get('approvals') or []
print('  approvals as the phone sees them:', len(a))
for x in a: print('   ', x.get('approval_id'), x.get('tool_name') or x.get('summary'), '| can_approve', x.get('can_approve'), '| conversation', x.get('conversation_id'))
EOF
AID=$(python3 -c "import json;d=json.load(open('$OUT/approvals.json'));r=d.get('response',d);a=(r.get('data') or {}).get('approvals') or r.get('approvals') or [];print(a[0]['approval_id'] if a else '')")
DIG=$(python3 -c "import json;d=json.load(open('$OUT/approvals.json'));r=d.get('response',d);a=(r.get('data') or {}).get('approvals') or r.get('approvals') or [];print(a[0]['digest'] if a else '')")
if [ -n "$AID" ]; then
  log "--- the phone says yes"
  control "$(cred agent.approve)" agent.approve "{\"approval_id\":\"$AID\",\"digest\":\"$DIG\"}" | tee "$OUT/approve.json" | say | tee -a "$LOG"
else
  log "  no permission request appeared in 120 s; the agent may have been allowed to write without asking"
fi
wait_idle "$C"; sleep 2
CTL agent read "$C" > "$OUT/read-c.json"; log "C has $(python3 -c "import json;print(json.load(open('$OUT/read-c.json'))['exchange_count'])") exchanges"
ls -la "/mnt/c/dev/../../tmp" >/dev/null 2>&1; log "file inside the distribution: $(ls -la /tmp/phone-$STAMP.txt 2>&1 | awk '{print $5, $9}') -> $(cat /tmp/phone-$STAMP.txt 2>/dev/null)"

log "--- the record"
EV="$DATA/fork/events/$C.jsonl"; cp "$EV" "$OUT/events-c.jsonl" 2>/dev/null
python3 - "$OUT/events-c.jsonl" <<'EOF' | tee -a "$LOG"
import json,sys
for line in open(sys.argv[1]):
    try: r=json.loads(line)
    except Exception: continue
    if r.get('event') in ('prompt_submit','permission_request','permission_replied','session_start','stop'):
        print('   ', r['ts'][11:23], r['event'], '| via', r.get('via'), '| answered_by', r.get('answered_by'), '| decision', r.get('decision'), '|', (r.get('summary') or '')[:50])
EOF
"$EXE" --warpctrl agent trace "$C" > "$OUT/trace-c.txt" 2>&1; log "trace lines mentioning the phone: $(grep -c 'from the phone' "$OUT/trace-c.txt")"; grep -n "from the phone" "$OUT/trace-c.txt" | head -4 | tee -a "$LOG"
"$EXE" --warpctrl agent trace "$C" --html "$OUT/trace-c.html" >/dev/null 2>&1; log "html trace mentions the phone: $(grep -c 'from the phone' "$OUT/trace-c.html")"

log "--- stop sharing from the desk: the phone is refused, the block goes"
CLICK 905 516 | tee -a "$LOG"; sleep 3; SHOT state-after-stop
control "$(cred agent.prompt 2>/dev/null || echo none)" agent.prompt "{\"prompt\":\"Reply with the word after.\",\"conversation_id\":\"$C\"}" 2>/dev/null | tee "$OUT/prompt-after-stop.json" | say | tee -a "$LOG"

log "--- a second conversation, the chip once, and two minutes: expired"
CTL agent prompt 'Say the single word second and nothing else.' > "$OUT/prompt-d.json"
D=$(python3 -c "import json;print(json.load(open('$OUT/prompt-d.json'))['conversation_id'])")
wait_idle "$D"; sleep 2
CLICK 905 516 | tee -a "$LOG"; sleep 3; SHOT state-waiting-d
log "waiting 125 s for the code to die unscanned"; sleep 125
SHOT state-expired
log "chip after the code died (should read /remote-control again): see state-expired.png"

log "--- close"
CTL window close > "$OUT/close.json"; sleep 8
N=$(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])))")
if [ "$N" != "0" ]; then log "close asked to confirm (a process was running); asking again"; CTL window close >/dev/null; sleep 8; N=$(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])))"); fi
log "after close: $N record(s)"
log "done"
