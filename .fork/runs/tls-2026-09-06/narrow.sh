#!/bin/bash
# Third pass: the block in a narrow view. The block is drawn in the panel's
# view of the conversation (block-after-click.png), so the panel is narrowed by
# splitting the pane twice with the panel still showing, no Escape.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver-narrow.log"; BIND=${BIND:-192.168.254.3:41234}
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
log "launch"; powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Console -Bind "$BIND" > "$OUT/launch-narrow.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 2
CTL tab create >/dev/null; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 3
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-narrow.json"
ID=$(python3 -c "import json;print(json.load(open('$OUT/prompt-narrow.json'))['conversation_id'])")
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$ID']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done; sleep 2
powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X 905 -Y 516 2>&1 | tail -1 | tee -a "$LOG"
sleep 2
CTL pane split --direction right >/dev/null; sleep 2; SHOT block-half
CTL pane split --direction right >/dev/null; sleep 2; SHOT block-third
log "close"; CTL window close >/dev/null; sleep 8; log "after close: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
