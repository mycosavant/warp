#!/bin/bash
# The four small AI features against a model on this machine, on the Windows
# release build after the validator fix. llama-server b10844 (CUDA 13.3) runs
# on the Windows side at 127.0.0.1:8080 with Gemma 4 12B UD-Q4_K_XL and
# thinking off (C:\dev\llama\serve.ps1); a scratch profile (a copy of the
# `wslauto` one, relocated, see SCRATCH below) declares it as a key-less
# Custom Inference endpoint in settings.toml:
#
#   [agents.local_ai]            endpoint = "llama"
#   [agents.custom_endpoints.llama-local]
#   name = "llama"  base_url = "http://127.0.0.1:8080/v1"
#   schema = "openai_chat_completions"
#   models = [{ name = "gemma-4-12b", config_key = "gemma-4-12b" }]
#
# Part 1 (this file): launch, a pane in a scratch repo with one uncommitted
# change, one command so Next Command has a block to work from, screenshots,
# and the server's request count before and after. The commit dialog is
# opened by hand from the screenshot, because its button has no warpctrl
# action and its coordinates come from the frame.
set -u
# The DEBUG build, on purpose. `WARP_DATA_PROFILE` is honoured by debug
# builds only (`ChannelState::data_profile`, `cfg!(debug_assertions)`), and
# a release build resolves its directories through the Windows known-folder
# API, so a relocated LOCALAPPDATA does not move it either. Two attempts
# at this run on the release binary each ran on the maintainer's real
# profile, never saw the endpoint, and sent the server nothing. A scratch
# profile on Windows means the debug binary, as the cancel run found.
EXE=/mnt/c/dev/warp/target/debug/warp-oss.exe
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=$(dirname "$0"); LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
DESK() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\'"$1"'.png' >/dev/null 2>&1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
KEY() { powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key "$1" "${@:2}" 2>&1 | tail -1; }
# One `launch_slot_` line per request, checked against four known requests.
requests() { grep -c "launch_slot_" /mnt/c/dev/llama/server.err 2>/dev/null || echo 0; }

log "local-model run, binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version)"
log "server: $(curl -s -m 3 http://127.0.0.1:8080/v1/models | tr -d ' \n' | head -c 80)"
log "server requests before: $(requests)"

# The profile rides the PowerShell environment: WSL passes nothing across
# the interop boundary unless WSLENV names it, so it is set inside.
setsid nohup powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "\$env:WARP_DATA_PROFILE='localai'; & 'C:\dev\warp\.fork\tools\warpdev.ps1' -Exe '$(echo "$EXE" | sed 's#/mnt/c/#C:/#; s#/#\\#g')'" > "$OUT/launch.txt" 2>&1 < /dev/null &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done; sleep 4
log "instance: $(CTL instance list | tr -d ' \n' | head -c 160)"

CTL tab create > "$OUT/tab.json"; sleep 2
CTL input submit 'cd /home/effatha/scratch-localai' >/dev/null; sleep 4
CTL input submit 'git status --short' >/dev/null; sleep 8
DESK local-1-after-command
log "server requests after one command: $(requests)"
sleep 6
DESK local-2-settled
log "server requests settled: $(requests)"
log "part 1 done; the commit dialog is driven by hand from local-2-settled.png"
