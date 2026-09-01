#!/usr/bin/env bash
# The horizon run: claude-agent-acp in `default`, instrumented, on the wire.
# Recipe notes that are load-bearing (CLAUDE.md):
#   - unset WAYLAND_DISPLAY  -> winit on X11, which screenshots and hotkeys need
#   - LIBGL_ALWAYS_SOFTWARE  -> WSLg
set -u
RUN="$(cd "$(dirname "$0")" && pwd)"
cd /home/effatha/git/warp

export WARP_FORK_ACP_COMMAND="$HOME/.npm/_npx/fca12915ff656968/node_modules/.bin/claude-agent-acp"
export WARP_FORK_ACP_MODE=default          # the mode where the agent asks
export WARP_FORK_EVENT_LOG="$RUN/events"   # one JSONL per conversation
export WARP_FORK_TRANSCRIPT=on             # -> .warp/transcripts under the pane cwd
export WARP_FORK_FRAME_LOG=on

exec env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/release/warp-oss
