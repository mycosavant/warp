#!/bin/bash
# Drive the routed commit dialog end to end, on the recommended configuration:
# Windows GUI, WSL pane auto-routed at bootstrap, commit message generated from
# a model on this machine. This is the live check `036bcccac` did not have.
#
# The 2026-09-07 rig, reused rather than reinvented
# (`.fork/runs/localmodel-2026-09-07/`): llama-server on the Windows side at
# 127.0.0.1:8080, and the `localai` scratch profile, which is a copy of
# `wslauto` with a key-less Custom Inference endpoint appended. The DEBUG
# binary, because `WARP_DATA_PROFILE` is honoured by debug builds only and a
# release build would run on the maintainer's real profile.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png" 2>/dev/null; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
# One `launch_slot_` line per request the model served.
requests() { grep -c "launch_slot_" /mnt/c/dev/llama/server.err 2>/dev/null || echo 0; }
WARPLOG=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss-localai/data/logs/warp-oss.log

case "${1:-launch}" in
launch)
  log "=== routed commit message, live ==="
  log "gui:    $(cat /mnt/c/dev/warp/target/debug/warp-oss.version 2>/dev/null)"
  log "daemon: $(cat /home/effatha/git/warp/target/release/warp-oss.version 2>/dev/null)"
  log "server: $(curl -s -m 3 http://127.0.0.1:8080/v1/models | tr -d ' \n' | head -c 100)"
  log "server requests before: $(requests)"

  setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -Command \
    "\$env:WARP_DATA_PROFILE='localai'; & 'C:\dev\warp\.fork\tools\warpdev.ps1' -Exe 'C:\dev\warp\target\debug\warp-oss.exe'" \
    > "$OUT/launch.txt" 2>&1 < /dev/null &
  for i in $(seq 1 40); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done
  sleep 5
  log "instance: $(CTL instance list | tr -d ' \n' | head -c 200)"

  CTL tab create > "$OUT/tab.json"; sleep 3
  CTL input submit 'cd /home/effatha/scratch-localai' >/dev/null; sleep 5
  # The whole claim rests on this pane being routed. `where: host` is routed;
  # `where: local` means the daemon was never asked and the run proves nothing.
  log "session inspect: $(CTL session inspect | tr -d ' \n' | head -c 300)"
  CTL input submit 'git status --short' >/dev/null; sleep 6
  DESK routed-1-pane
  log "server requests after the pane: $(requests)"
  ;;
review)
  log "code review panel: $(CTL surface code-review open | tr -d ' \n' | head -c 120)"
  sleep 6
  DESK routed-2-code-review
  log "server requests after the panel: $(requests)"
  ;;
commit)
  # $2,$3: the Commit button, read off routed-2-code-review.png
  log "commit button at $2,$3: $(CLICK "$2" "$3")"; sleep 3
  DESK routed-3-dialog-opening
  sleep 12
  DESK routed-4-dialog
  log "server requests after the dialog: $(requests)"
  grep -n "generate_code_review_content\|commit message\|No AI endpoint\|GenerateCommitMessage" "$WARPLOG" | tail -8 | tee -a "$LOG"
  ;;
esac
