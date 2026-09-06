#!/bin/bash
# Second pass: the block itself. The first pass's click landed on *Stop
# sharing*, because the curl device was already paired for the conversation;
# this pass clicks once on an unpaired conversation, so the chip starts.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver-block.log"; BIND=${BIND:-192.168.254.3:41234}
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
log "launch"; powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Console -Bind "$BIND" > "$OUT/launch-block.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 2
CTL tab create >/dev/null; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 3
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-block.json"
ID=$(python3 -c "import json;print(json.load(open('$OUT/prompt-block.json'))['conversation_id'])")
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$ID']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done; sleep 2
log "conversation $ID; the chip, once"
SHOT block-footer
powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X 905 -Y 516 2>&1 | tail -1 | tee -a "$LOG"
sleep 2; SHOT block-after-click
powershell.exe -NoProfile -Command "Get-Clipboard" > "$OUT/clipboard-block.txt" 2>&1; log "clipboard: $(tr -d '\r' < "$OUT/clipboard-block.txt" | sed 's/#.*/#<code>/')"
powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key Escape 2>&1 | tail -1
sleep 2; SHOT block-wide
CTL pane split --direction right >/dev/null; sleep 1; CTL pane split --direction right >/dev/null; sleep 2
SHOT block-narrow
log "close"; CTL window close >/dev/null; sleep 8; log "after close: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
