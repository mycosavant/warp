#!/usr/bin/env bash
# The horizon run: claude-agent-acp in `default`, instrumented, on the wire.
# Recipe notes that are load-bearing (CLAUDE.md):
#   - unset WAYLAND_DISPLAY  -> winit on X11, which screenshots and hotkeys need
#   - LIBGL_ALWAYS_SOFTWARE  -> WSLg
set -u
RUN="$(cd "$(dirname "$0")" && pwd)"
cd /home/effatha/git/warp

# NOTE (added 2026-09-04): the path below is an `npx` *cache key*, not a version
# pin -- npx hashes the spec string you typed, so nothing in the path says which
# version it holds. This one held 0.70.0, which is what this run used and what
# friction.md records. The directory was deleted on 2026-09-04 during a cleanup
# that left one agent at one version, so re-running this script as written will
# fail to spawn the agent. To reproduce the run, name the version instead:
#   export WARP_FORK_ACP_COMMAND="npx -y @agentclientprotocol/claude-agent-acp@0.70.0"
# Deliberately not rewritten to 0.73.0: this file is a record of what ran.
export WARP_FORK_ACP_COMMAND="$HOME/.npm/_npx/fca12915ff656968/node_modules/.bin/claude-agent-acp"
export WARP_FORK_ACP_MODE=default          # the mode where the agent asks
export WARP_FORK_EVENT_LOG="$RUN/events"   # one JSONL per conversation
export WARP_FORK_TRANSCRIPT=on             # -> .warp/transcripts under the pane cwd
export WARP_FORK_FRAME_LOG=on

exec env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/release/warp-oss
