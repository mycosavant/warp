#!/bin/bash
# The chip itself: click /remote-control in the footer of a pane whose panel
# has a conversation, read the link the click copied, open it in Brave.
set -u
OUT=${1:?outdir}
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out "C:\dev\shots\$1.png" 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$OUT/driver.log"; }
log "before the click"; SHOT rc-before
