#!/bin/bash
# The top of the conversation view: the header line with mode, model, agent
# and directory read off the record. The second pass's screenshot had scrolled
# to the newest rows.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
BRAVE='C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe'
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver-head.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process "${2:-warp-oss}" -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
log "launch"; powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch-head.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 3
CTL tab create >/dev/null; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 3
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-head.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-head.json'))['conversation_id'])")
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$C']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done
CTL pair show --conversation "$C" > "$OUT/pair-show-head.json"
BURL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show-head.json'))['url'])")
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null; sleep 1
powershell.exe -NoProfile -Command "Start-Process -FilePath '$BRAVE' -ArgumentList '--user-data-dir=C:\dev\brave-scratch-head','--no-first-run','--ignore-certificate-errors','--window-size=430,1000','--window-position=40,40','$BURL'" 2>&1 | tail -1
sleep 8; powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process brave -Key Home -Ctrl 2>&1 | tail -1; sleep 2; SHOT page-head brave
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null
log "close"; CTL window close >/dev/null; sleep 8; log "after close: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])))") record(s)"
