#!/bin/bash
# Viewer phase 0, second half: does the join hold on a live product-profile session under `auto`?
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
WEXE='C:\dev\warp\target\debug\warp-oss.exe'
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
EVENTS=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/fork/events
OUT=${1:?outdir}; mkdir -p "$OUT"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$OUT/driver.log"; }

log "launching PRODUCT + EVENT LOG"
powershell.exe -NoProfile -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -EventLog -Exe "$WEXE" 2>&1 | tee "$OUT/launch.txt"
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q inst_ && break; done
CTL instance list | tee "$OUT/instance.json"
sleep 6
log "cd the pane"
CTL input submit 'cd /home/effatha/git/warp' | tee -a "$OUT/warpctrl.txt"; echo >> "$OUT/warpctrl.txt"
sleep 4
CTL session inspect | tee "$OUT/session-inspect.json"
log "prompt"
PROMPT='Use your Read tool to read the first line of CLAUDE.md and quote it exactly. Then run `git log --oneline -1` with your Bash tool and quote its output. Two tool calls, then answer in two lines.'
CTL agent prompt "$PROMPT" | tee -a "$OUT/warpctrl.txt"
CID=$(grep -o '"conversation_id": *"[^"]*"' "$OUT/warpctrl.txt" | tail -1 | sed 's/.*: *"//;s/"//')
log "conversation $CID"
for i in $(seq 1 60); do
  sleep 3
  BUSY=$(CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d.get('conversations',[]) if c['conversation_id']=='$CID']; print(c[0]['is_busy'] if c else 'none')")
  [ "$BUSY" = "False" ] && break
done
log "turn over after ~$((i*3))s, is_busy=$BUSY"
CTL agent list | tee "$OUT/agent-list.json" >/dev/null
CTL agent read "$CID" | tee "$OUT/agent-read.json" >/dev/null
cp "$EVENTS/$CID.jsonl" "$OUT/events.jsonl" 2>/dev/null || log "NO EVENT FILE $EVENTS/$CID.jsonl"
log "event kinds:"; grep -o '"event":"[^"]*"' "$OUT/events.jsonl" | sort | uniq -c | tee -a "$OUT/driver.log"
LINKED=$(grep -o '"linked_session_id":"[^"]*"' "$OUT/events.jsonl" | sort -u | tee -a "$OUT/driver.log" | head -1 | sed 's/.*:"//;s/"//')
log "linked_session_id = $LINKED"
HF=~/.claude/projects/-home-effatha-git-warp/$LINKED.jsonl
if [ -f "$HF" ]; then cp "$HF" "$OUT/harness-session.jsonl"; log "harness file: $(wc -l < "$HF") lines"; else log "NO HARNESS FILE $HF"; fi
log "join on call_id:"
for id in $(grep -o '"call_id":"[^"]*"' "$OUT/events.jsonl" | sed 's/.*:"//;s/"//' | sort -u); do
  n=$(grep -c "\"$id\"" "$HF" 2>/dev/null || echo 0); log "  $id  in harness file: $n lines"
done
log "permission lines: $(grep -c permission "$OUT/events.jsonl")"
powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out "C:\dev\shots\phase0-$CID.png" 2>&1 | tail -1
log "closing"; CTL window close | tail -3
