#!/bin/bash
# The checklist at the top of .fork/HANDOFF-MOBILE.md, run against the
# Android emulator (`.fork/tools/phone.sh`, a Windows process, so it reaches
# the wide listener the way a phone on the LAN does). Assumes the authority
# is already installed on the emulator and notifications already allowed for
# the origin, both done by hand earlier the same evening and recorded in the
# README; this pass is on the binary that posts notifications through the
# worker. The rig profile (-Instrumented) puts the agent in `default` mode so
# a Write asks, which is what the phone is told about and answers.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
P=/home/effatha/git/warp/.fork/tools/phone.sh
DATA=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
snap() { $P shot "$OUT/$1" >/dev/null; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }
approvals() { CTL agent approvals | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("approvals",[])))'; }
events() { python3 - "$1" <<'EOF'
import json,sys
for line in open(sys.argv[1]):
    try: r=json.loads(line)
    except Exception: continue
    if r.get('event') in ('prompt_submit','permission_request','permission_replied','stop','stop_failure'):
        print('   ', r['ts'][11:23], r['event'], '| via', r.get('via'), '| answered_by', r.get('answered_by'), '| decision', r.get('decision'), '|', (r.get('summary') or '')[:50])
EOF
}
chrome_notifications() { $P adb shell 'dumpsys notification --noredact 2>/dev/null' | grep -c 'pkg=com.android.chrome'; }

log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version); emulator $($P adb shell getprop ro.build.version.release | tr -d '\n') (API $($P adb shell getprop ro.build.version.sdk | tr -d '\n')), Chrome $($P adb shell dumpsys package com.android.chrome | grep versionName | head -1 | tr -d ' \n')"
log "instances before: $(CTL instance list | python3 -c "import sys,json;print(len(json.load(sys.stdin).get('instances',[])))")"
log "launch: warpdev.ps1 -Instrumented -Console -Bind $BIND"
setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch.txt" 2>&1 < /dev/null &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 3
log "$(grep -a 'wide listener' $DATA/logs/warp-oss.log | tail -1 | cut -c1-120)"
$P adb shell am force-stop com.android.chrome; sleep 1

log "--- a conversation, the chip once, the link opened on the phone"
CTL tab create > "$OUT/tab.json"; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-c.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-c.json'))['conversation_id'])"); echo "$C" > "$OUT/conversation-c.txt"; log "conversation C $C"; wait_idle "$C"; sleep 2
CLICK 905 516 | tee -a "$LOG"; sleep 3; DESK desk-waiting
URL=$(powershell.exe -NoProfile -Command "Get-Clipboard" | tr -d '\r\n'); log "clipboard: ${URL%%#*}#<code>"
$P open "$URL" >/dev/null; sleep 8; snap p1-paired
log "  chrome says: $($P ui | python3 -c "
import sys,xml.etree.ElementTree as ET
for n in ET.fromstring(sys.stdin.read()).iter('node'):
    if 'secure' in n.get('content-desc','').lower(): print(n.get('content-desc')); break" )"
sleep 4; DESK desk-paired

log "--- Chrome in the background (HOME): the agent asks, and Android does not run the page"
$P key HOME >/dev/null; sleep 2; BEFORE=$(chrome_notifications); log "chrome notification records before: $BEFORE"
STAMP=$(date +%s); echo "$STAMP" > "$OUT/stamp.txt"
CTL agent prompt --conversation "$C" "Create the file /tmp/phone-$STAMP.txt containing the single word hello. Use your Write tool. Say done when it exists." > "$OUT/prompt-write.json"
for i in $(seq 1 20); do sleep 3; N=$(chrome_notifications); [ "${N:-0}" -gt "$BEFORE" ] && break; done; log "chrome notification records after ~$((i*3)) s: $N (approvals waiting: $(approvals))"
log "  the desk answers this one, so the next ask is fresh"
A=$(CTL agent approvals | python3 -c 'import sys,json;a=json.load(sys.stdin).get("approvals",[]);print(a[0]["approval_id"] if a else "")'); D=$(CTL agent approvals | python3 -c 'import sys,json;a=json.load(sys.stdin).get("approvals",[]);print(a[0]["digest"] if a else "")')
[ -n "$A" ] && CTL agent approve "$A" --digest "$D" >/dev/null; wait_idle "$C"

log "--- the tab hidden behind another tab, Chrome in the foreground: the phone is told"
$P adb shell am start -n com.android.chrome/com.google.android.apps.chrome.Main >/dev/null 2>&1; sleep 2; $P open 'about:blank' >/dev/null; sleep 3
BEFORE=$(chrome_notifications); STAMP=$(date +%s); echo "$STAMP" > "$OUT/stamp.txt"
CTL agent prompt --conversation "$C" "Create the file /tmp/phone-$STAMP.txt containing the single word hello. Use your Write tool. Say done when it exists." > "$OUT/prompt-write-2.json"
for i in $(seq 1 30); do sleep 3; N=$(chrome_notifications); [ "${N:-0}" -gt "$BEFORE" ] && break; done; log "chrome notification records after ~$((i*3)) s: $N (before $BEFORE; approvals waiting: $(approvals))"
$P adb shell 'dumpsys notification --noredact 2>/dev/null' | grep -A14 'pkg=com.android.chrome' | grep -E 'android.title=|android.text=' | grep -v -i download | head -2 | sed 's/^ */    /' | tee -a "$LOG"
$P adb shell cmd statusbar expand-notifications; sleep 2; snap p2-shade
log "  tapping it: $($P tapon "asks:" 2>&1 | tail -1)"; sleep 3; log "  lands in: $($P adb shell 'dumpsys window | grep mCurrentFocus' | sed 's/^ *//')"; snap p2b-after-tap

log "--- the phone answers: scrolled to the top, a tap arms Yes, a second tap sends it"
for i in $(seq 1 14); do $P adb shell input swipe 540 500 540 1900 300; sleep 0.4; done; sleep 2; snap p3-ask
$P tap 280 1130; sleep 0.7; snap p4-armed; $P tap 280 1130; sleep 3; snap p5-answered; log "approvals after the phone's yes: $(approvals)"
wait_idle "$C"; log "file inside the distribution: $(ls -la /tmp/phone-$STAMP.txt 2>&1 | awk '{print $5,$9}') -> $(cat /tmp/phone-$STAMP.txt 2>/dev/null)"

log "--- a prompt from the box"
$P tap 540 2111; sleep 2; $P adb shell input text 'Reply%swith%sthe%ssingle%sword%sphone.'; sleep 1; $P key BACK >/dev/null; sleep 1; snap p6-typed
$P tap 200 2278; sleep 4; wait_idle "$C"; snap p7-replied

log "--- the record"
EV="$DATA/fork/events/$C.jsonl"; cp "$EV" "$OUT/events-c.jsonl"; events "$EV" | tail -6 | tee -a "$LOG"
"$EXE" --warpctrl agent trace "$C" --harness-dir '\\wsl.localhost\Ubuntu\home\effatha\.claude\projects' > "$OUT/trace-c.txt" 2>&1; log "trace lines mentioning the phone: $(grep -c 'from the phone' "$OUT/trace-c.txt")"
DESK desk-after-phone

log "--- the installed app, from the home screen (installed by hand earlier; Chrome's menu offered Add to Home screen and the sheet read Install app)"
$P key HOME >/dev/null; sleep 2; $P tapon "warp" | tee -a "$LOG"; sleep 6; log "  focus: $($P adb shell 'dumpsys window | grep mCurrentFocus' | sed 's/^ *//')"; snap p8-standalone

log "--- stop sharing from the desk: the phone is refused"
$P adb shell am start -n com.android.chrome/com.google.android.apps.chrome.Main >/dev/null 2>&1; sleep 2
CLICK 905 516 | tee -a "$LOG"; sleep 4; DESK desk-after-stop; sleep 6; snap p12-after-stop

log "--- a second conversation handed over and left paired, for the morning"
$P adb shell am start -n com.android.chrome/com.google.android.apps.chrome.Main >/dev/null 2>&1; sleep 2
CTL agent prompt 'Say the single word overnight and nothing else.' > "$OUT/prompt-d.json"
D=$(python3 -c "import json;print(json.load(open('$OUT/prompt-d.json'))['conversation_id'])"); echo "$D" > "$OUT/conversation-d.txt"; wait_idle "$D"; sleep 2
CLICK 905 516 | tee -a "$LOG"; sleep 3; URL=$(powershell.exe -NoProfile -Command "Get-Clipboard" | tr -d '\r\n'); $P open "$URL" >/dev/null; sleep 8; snap p13-overnight; DESK desk-overnight
log "conversation D $D paired at $(date -u +%H:%M:%S); Warp and the emulator are left running"
