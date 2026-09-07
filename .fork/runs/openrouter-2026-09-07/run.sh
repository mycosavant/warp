#!/bin/bash
# T21.3b / I22: the picker against opencode's OpenRouter list, on the
# Windows release build. `warpdev.ps1 -Agent` names opencode for this launch,
# started inside the distribution so the pane's cwd resolves. Parts, because
# the picker is driven by coordinates read off screenshots between them:
#   part1        launch, one turn, the chip photographed, the picker opened
#   search WORD  type WORD into the picker's search box and photograph
#   pick X Y     click a row, run a second turn asking the model its name
#   part3        the store, the event log, close
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
DATA=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data
# The absolute path, not `opencode`: a login shell started by wsl.exe from
# Windows sources .profile, whose .bashrc returns for a non-interactive shell
# before it loads nvm, so `which opencode` is empty there and the first
# attempt closed its transport at `initialize` (attempt-1 screenshots).
AGENT='wsl.exe -d Ubuntu --shell-type login -- /home/effatha/.nvm/versions/node/v24.5.0/bin/opencode acp'
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
KEY() { powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key "$1" 2>&1 | tail -1; }
TYPE() { powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Text "$1" 2>&1 | tail -1; }
wait_idle() { for i in $(seq 1 60); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 180 s)"; }
model_lines() { grep -h "session_model\|session_agent" "$DATA/fork/events/$1.jsonl" | python3 -c 'import sys,json
for l in sys.stdin:
    r=json.loads(l); print("   ", r["ts"][11:19], r["summary"])'; }

case "${1:-}" in
part1)
  log "openrouter run, binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version), agent: $AGENT"
  setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Instrumented -Agent "$AGENT" > "$OUT/launch.txt" 2>&1 < /dev/null &
  for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
  CTL tab create > "$OUT/tab.json"; sleep 2; CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
  CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-1.json"
  C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-1.json'))['conversation_id'])"); echo "$C" > "$OUT/conversation.txt"; log "conversation $C"
  wait_idle "$C"; sleep 2; DESK or-1-after-turn
  log "the chip clicked at 647,873: $(CLICK 647 873)"; sleep 3; DESK or-2-picker-open
  log "session lines:"; model_lines "$C" | tee -a "$LOG"
  python3 -c "
import json; d=json.load(open('$DATA/fork/acp-models.json')); print('    store rows:', len(d['choices']), 'default', d['default_id']); print('    first five:', [c['display_name'] for c in d['choices'][:5]])" | tee -a "$LOG"
  ;;
search)
  log "typing '$2' into the search box: $(TYPE "$2")"; sleep 2; DESK "or-3-search-$2"
  ;;
pick)
  C=$(cat "$OUT/conversation.txt")
  log "a row clicked at $2,$3: $(CLICK "$2" "$3")"; sleep 2; DESK or-4-picked
  CTL agent prompt --conversation "$C" 'Which model are you? Reply with your model id only.' > "$OUT/prompt-2.json"
  wait_idle "$C"; sleep 2; DESK or-5-after-second-turn
  log "session lines:"; model_lines "$C" | tee -a "$LOG"
  ;;
part3)
  cp "$DATA/fork/acp-models.json" "$OUT/acp-models.json"
  C=$(cat "$OUT/conversation.txt"); cp "$DATA/fork/events/$C.jsonl" "$OUT/events.jsonl" 2>/dev/null
  CTL window close > "$OUT/close.json"; sleep 6; log "instances after close: $(CTL instance list | tr -d ' \n' | head -c 120)"
  ;;
esac
