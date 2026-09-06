#!/bin/bash
# Item 5's other half, measured: the product profile on the build with
# HandoffLocalCloud in FORCE_DISABLED. Expect no "Hand off to cloud" chip in
# the agent footer and no /move-to-cloud in the slash registry.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version)"
log "launch, product profile, detached"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' > "$OUT/launch.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done
log "instance: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
CTL tab create > "$OUT/tab.json"; sleep 2
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 3
CTL slash list > "$OUT/slash-list.json"
log "slash registry: $(python3 -c "import json;d=json.load(open('$OUT/slash-list.json'));c=d.get('commands',d);names=[x.get('name',x) if isinstance(x,dict) else x for x in (c if isinstance(c,list) else [])];print(len(names),'commands;','move-to-cloud PRESENT' if any('move-to-cloud' in str(n) for n in names) else 'no move-to-cloud')")"
log "one conversation, so the agent footer is drawn"
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt.json"
ID=$(python3 -c "import json;print(json.load(open('$OUT/prompt.json'))['conversation_id'])")
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$ID']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done
sleep 2
powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\footer-release.png' 2>&1 | tail -1 | tee -a "$LOG"
cp /mnt/c/dev/shots/footer-release.png "$OUT/footer-release.png"
log "screenshot taken; closing"
CTL window close > "$OUT/close.json"; sleep 6
log "after close: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
