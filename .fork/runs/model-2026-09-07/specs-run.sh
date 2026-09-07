#!/bin/bash
# The specs card after `5856078d9`: the table's bars, the fork's header,
# the tagline and the price under it, and the one word in the menu. Same
# rig profile as label-run.sh; one turn, the chip clicked, then each row
# hovered (click.ps1 -Hover, a move with no button) and the card
# photographed for it. Row coordinates are read off label-3-picker-open.png
# and must be re-read if the layout moves. The per-row cards are reached
# by arrow keys, because a hover does not move the details pane.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
HOVER() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" -Hover 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }

log "specs run, binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version)"
setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch-specs.txt" 2>&1 < /dev/null &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
CTL tab create > "$OUT/tab-specs.json"; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-specs.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-specs.json'))['conversation_id'])"); log "conversation S $C"
wait_idle "$C"; sleep 2; DESK specs-1-after-turn
log "the chip clicked at 647,873: $(CLICK 647 873)"; sleep 2; DESK specs-2-picker-open
# A hover highlights the row and leaves the details pane on the selected
# item (measured, first run of this script); the pane follows the keyboard.
# Sonnet is the fourth of five rows, so Up three times reaches the top.
KEY() { powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key "$1" 2>&1 | tail -1; }
KEY Up; KEY Up; KEY Up; sleep 1
for row in default opus fable sonnet haiku; do
  sleep 1; DESK "specs-3-$row"; log "card for $row photographed; $(KEY Down)"
done
powershell.exe -NoProfile -Command '[System.Windows.Forms.SendKeys]::SendWait("{ESC}")' 2>/dev/null; sleep 1
cp /mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/fork/acp-models.json "$OUT/acp-models-specs.json"
python3 - "$OUT" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]+"/acp-models-specs.json"))
for c in d["choices"]: print("   store:", c["id"], "|", c["display_name"], "|", c["description"], "|", c["spec"])
PY
CTL window close > "$OUT/close-specs.json"; sleep 6; log "instances after close: $(CTL instance list | tr -d ' \n' | head -c 120)"
