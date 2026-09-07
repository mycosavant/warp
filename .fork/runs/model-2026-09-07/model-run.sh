#!/bin/bash
# The model picker (T14.14, second half) and the stop notification's body,
# on the Windows release build. The rig profile, wide listener, so the
# emulator can be told. Clicks in the picker are by coordinates read off
# screenshots between steps, so this is run in parts: `part1` launches and
# runs a first turn; `part2 <x> <y>` clicks a picker row and runs the second
# turn; `part3` measures the notification on the emulator.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
P=/home/effatha/git/warp/.fork/tools/phone.sh
DATA=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }
model_lines() { grep -h session_model "$DATA/fork/events/$1.jsonl" | python3 -c 'import sys,json
for l in sys.stdin:
    r=json.loads(l); print("   ", r["ts"][11:19], r["summary"])'; }
snap() { $P shot "$OUT/$1" >/dev/null; }

case "${1:-}" in
part1)
  log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version)"
  log "launch: warpdev.ps1 -Instrumented -Console -Bind $BIND"
  setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Console -Bind "$BIND" > "$OUT/launch.txt" 2>&1 < /dev/null &
  for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
  CTL tab create > "$OUT/tab.json"; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
  CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-1.json"
  C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-1.json'))['conversation_id'])"); echo "$C" > "$OUT/conversation-c.txt"; log "conversation C $C"
  sleep 2; DESK desk-1-during-first-turn
  wait_idle "$C"; sleep 2; DESK desk-2-after-first-turn
  log "session_model lines:"; model_lines "$C" | tee -a "$LOG"
  ;;
part2)
  C=$(cat "$OUT/conversation-c.txt")
  log "the chip clicked at $2,$3: $(CLICK "$2" "$3")"; sleep 2; DESK desk-3-picker-open
  ;;
pick)
  C=$(cat "$OUT/conversation-c.txt")
  log "a row clicked at $2,$3: $(CLICK "$2" "$3")"; sleep 2; DESK desk-4-picked
  CTL agent prompt --conversation "$C" 'Which model are you? Reply with your model id only.' > "$OUT/prompt-2.json"
  wait_idle "$C"; sleep 2; DESK desk-5-after-second-turn
  log "session_model lines:"; model_lines "$C" | tee -a "$LOG"
  "$EXE" --warpctrl agent trace "$C" --harness-dir '\\wsl.localhost\Ubuntu\home\effatha\.claude\projects' > "$OUT/trace-c.txt" 2>&1
  log "the harness's own model field per assistant message:"; python3 - "$C" <<'PY' | tee -a "$LOG"
import json,glob,sys,os
c=sys.argv[1]
ev=[json.loads(l) for l in open(f"/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/fork/events/{c}.jsonl")]
linked={r.get("linked_session_id") for r in ev if r.get("linked_session_id")}
for sid in linked:
    for f in glob.glob(os.path.expanduser(f"~/.claude/projects/*/{sid}.jsonl")):
        for l in open(f):
            try: r=json.loads(l)
            except Exception: continue
            if r.get("type")=="assistant": print("   ", r["timestamp"][11:19], r["message"].get("model"))
PY
  ;;
part3)
  C=$(cat "$OUT/conversation-c.txt")
  CTL pair show --conversation "$C" > "$OUT/pair-show.json"
  URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show.json'))['url'])")
  $P adb shell am force-stop com.android.chrome; sleep 1
  $P open "$URL" >/dev/null; sleep 8; snap p1-paired
  $P adb shell am start -n com.android.chrome/com.google.android.apps.chrome.Main >/dev/null 2>&1; sleep 2; $P open 'about:blank' >/dev/null; sleep 3
  BEFORE=$($P adb shell 'dumpsys notification --noredact 2>/dev/null' | grep -c 'pkg=com.android.chrome')
  CTL agent prompt --conversation "$C" 'Reply with exactly this sentence and nothing else: The quick brown fox jumps over the lazy dog.' > "$OUT/prompt-3.json"
  for i in $(seq 1 30); do sleep 3; N=$($P adb shell 'dumpsys notification --noredact 2>/dev/null' | grep -c 'pkg=com.android.chrome'); [ "${N:-0}" -gt "$BEFORE" ] && break; done
  log "chrome notification records after ~$((i*3)) s: $N (before $BEFORE)"
  $P adb shell 'dumpsys notification --noredact 2>/dev/null' | grep -A14 'pkg=com.android.chrome' | grep -E 'android.title=|android.text=' | grep -v -i download | head -2 | sed 's/^ */    /' | tee -a "$LOG"
  $P adb shell cmd statusbar expand-notifications; sleep 2; snap p2-shade; $P adb shell cmd statusbar collapse
  ;;
esac
