#!/bin/bash
# Drive the routed PR-content path end to end. Adapted from
# `.fork/runs/routedcommit-2026-09-12/run.sh` rather than rewritten.
#
# The constraint that shapes it: NO REAL PULL REQUEST. `~/scratch-localai`'s
# `origin` is a bare repo on disk, so `gh` refuses before any network call --
# "none of the git remotes configured for this repository point to a known
# GitHub host", confirmed by hand before the run. The model request must still
# fire on THIS side first, and that is the whole claim under test.
set -u
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png" 2>/dev/null; }
CLICK() { powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X "$1" -Y "$2" 2>&1 | tail -1; }
requests() { grep -c "launch_slot_" /mnt/c/dev/llama/server.err 2>/dev/null || echo 0; }
WARPLOG=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss-localai/data/logs/warp-oss.log
REPO=/home/effatha/scratch-localai

case "${1:-launch}" in
launch)
  log "=== routed PR content, live ==="
  log "gui:    $(cat /mnt/c/dev/warp/target/debug/warp-oss.version 2>/dev/null)"
  log "daemon: $(cat /home/effatha/git/warp/target/release/warp-oss.version 2>/dev/null)"
  log "server: $(curl -s -m 3 http://127.0.0.1:8080/v1/models | tr -d ' \n' | head -c 100)"
  log "repo branch: $(git -C $REPO branch --show-current), ahead of main by $(git -C $REPO rev-list --count origin/main..HEAD)"
  log "gh refuses this repo: $(cd $REPO && gh repo view 2>&1 | head -c 80)"
  log "server requests before: $(requests)"

  setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -Command \
    "\$env:WARP_DATA_PROFILE='localai'; & 'C:\dev\warp\.fork\tools\warpdev.ps1' -Exe 'C:\dev\warp\target\debug\warp-oss.exe'" \
    > "$OUT/launch.txt" 2>&1 < /dev/null &
  for i in $(seq 1 40); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done
  sleep 5
  log "instance: $(CTL instance list | tr -d ' \n' | head -c 200)"

  CTL tab create > "$OUT/tab.json"; sleep 3
  CTL input submit "cd $REPO" >/dev/null; sleep 5
  # Routed or the run proves nothing: `where: host` is routed, `where: local`
  # means the daemon was never asked.
  log "session inspect: $(CTL session inspect | tr -d ' \n' | head -c 300)"
  CTL input submit 'git status --short' >/dev/null; sleep 6
  DESK pr-1-pane
  log "server requests after the pane: $(requests)"
  ;;
dirty)
  # A marker that cannot predate the run, for the commit half of the chain.
  MARK="chain_probe_$(date -u +%Y%m%d_%H%M%S)"
  cat >> $REPO/main.rs <<EOF

/// Added $(date -u +%Y-%m-%dT%H:%M:%SZ) by the routed PR drive.
fn $MARK(seed: u64) -> u64 {
    seed ^ (seed >> 33)
}
EOF
  log "uncommitted marker: $MARK"
  CTL input submit 'git status --short' >/dev/null; sleep 4
  ;;
review)
  log "code review panel: $(CTL surface code-review open | tr -d ' \n' | head -c 120)"
  sleep 6
  DESK pr-2-code-review
  log "server requests after the panel: $(requests)"
  ;;
click)
  # $2,$3 read off the previous screenshot; $4 names the shot.
  log "click $2,$3 ($4): $(CLICK "$2" "$3")"; sleep 3
  DESK "$4-opening"
  sleep 12
  DESK "$4"
  log "server requests after $4: $(requests)"
  ;;
log)
  log "--- warp log tail ---"
  grep -n "generate_code_review_content\|CreatePr\|create_pr\|commit message\|No AI endpoint\|GenerateCommitMessage\|gh pr\|GitHub host" "$WARPLOG" | tail -20 | tee -a "$LOG"
  ;;
esac
