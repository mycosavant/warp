#!/bin/bash
# Part 2: the commit-message feature. Assumes part 1 left Warp running with
# the active pane in ~/scratch-localai (one uncommitted change). Opens the
# code review panel with its shortcut (ctrl shift +, the hint row's own
# wording), photographs it so the Commit button can be found, and if a
# button position is given, clicks it and photographs the dialog after the
# model has had time to answer. The count of requests the server has served
# is read before and after each step.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
KEY() { powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key "$1" "${@:2}" 2>&1 | tail -1; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
requests() { grep -c "launch_slot_" /mnt/c/dev/llama/server.err 2>/dev/null || echo 0; }

case "${1:-open}" in
  open)
    log "part 2: requests before: $(requests)"
    log "code review panel: $(KEY Plus -Ctrl -Shift)"; sleep 5
    DESK local-3-code-review
    log "requests after opening the panel: $(requests)"
    ;;
  commit)
    # $2,$3: the Commit button, read off local-3-code-review.png
    log "commit button at $2,$3: $(CLICK "$2" "$3")"; sleep 2
    DESK local-4-commit-dialog-opening
    sleep 8
    DESK local-5-commit-dialog
    log "requests after the dialog: $(requests)"
    grep -n "generate_code_review_content\|Failed to generate\|commit message" /mnt/c/Users/onemind/AppData/Local/warp/WarpOss-localai/data/logs/warp-oss.log | tail -5 | tee -a "$LOG"
    ;;
esac
