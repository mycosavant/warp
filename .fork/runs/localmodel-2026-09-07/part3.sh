#!/bin/bash
# Part 3: the same commit dialog in an UNROUTED WSL pane, as the control for
# part 2. Routed, the daemon inside the distribution generates the message
# with its own AIClient, in a process where the fork's local AI config was
# never installed, and the dialog stays blank with "No AI endpoint is
# configured" in the log. Unrouted, the diff state is Local and the GUI
# process generates. Same profile, same server, one variable.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
requests() { grep -c "launch_slot_" /mnt/c/dev/llama/server.err 2>/dev/null || echo 0; }
WLOG=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss-localai/data/logs/warp-oss.log

case "${1:-launch}" in
  launch)
    CTL window close >/dev/null; sleep 8
    log "part 3: instances after close: $(CTL instance list | tr -d ' \n' | head -c 40)"
    log "requests before: $(requests)"
    setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "\$env:WARP_DATA_PROFILE='localai'; \$env:WARP_FORK_WSL_AUTO_CONNECT='0'; & 'C:\dev\warp\.fork\tools\warpdev.ps1' -Exe 'C:\dev\warp\target\debug\warp-oss.exe'" > "$OUT/launch3.txt" 2>&1 < /dev/null &
    for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
    CTL tab create > "$OUT/tab3.json"; sleep 2
    CTL input submit 'cd /home/effatha/scratch-localai' >/dev/null; sleep 4
    CTL input submit 'git status --short' >/dev/null; sleep 8
    log "session: $(CTL session inspect | tr -d ' \n' | grep -o '"filesystem":{[^}]*}')"
    CTL surface code-review open >/dev/null; sleep 6
    DESK local-6-unrouted-code-review
    log "requests after the panel: $(requests)"
    ;;
  commit)
    log "commit button at $2,$3: $(CLICK "$2" "$3")"; sleep 10
    DESK local-7-unrouted-commit-dialog
    log "requests after the dialog: $(requests)"
    grep -n "Failed to autogenerate\|commit message\|generate_code_review" "$WLOG" | tail -3 | tee -a "$LOG"
    ;;
esac
