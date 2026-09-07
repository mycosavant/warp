#!/bin/bash
# The chip's label after `8a6e64f81`: the version before ` · `, no tagline.
# Same rig profile and wide listener as model-run.sh; one turn, the chip
# photographed after it, the picker opened by the coordinates model-run.sh
# used and photographed, then the window closed.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }

log "label run, binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version)"
setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch-label.txt" 2>&1 < /dev/null &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
CTL tab create > "$OUT/tab-label.json"; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
DESK label-1-before-any-turn
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-label.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-label.json'))['conversation_id'])"); log "conversation L $C"
wait_idle "$C"; sleep 2; DESK label-2-after-turn
log "the chip clicked at 70,872: $(CLICK 70 872)"; sleep 2; DESK label-3-picker-open
powershell.exe -NoProfile -Command '[System.Windows.Forms.SendKeys]::SendWait("{ESC}")' 2>/dev/null; sleep 1
python3 - "$OUT" <<'PY'
import json,sys,glob
out=sys.argv[1]
d=json.load(open(glob.glob('/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/fork/acp-models.json')[0]))
print("   store:", d["default_id"], [c["display_name"] for c in d["choices"]], [c["description"] for c in d["choices"]])
PY
