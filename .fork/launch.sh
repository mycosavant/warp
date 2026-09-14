#!/usr/bin/env bash
# The daily driver. Everything under `.fork/runs/` is a measurement rig; this
# is the one for getting work done.
#
# The difference between them is a single variable, and it is the reason a
# working session was raising ~50 permission requests in half an hour.
#
#   WARP_FORK_ACP_MODE=default   <- the rigs set this. It asks the agent to run
#                                   in the mode where *everything* not on the
#                                   allow list raises a request, so the asks can
#                                   be counted. That is the instrument, not the
#                                   product.
#
# Here it is deliberately UNSET, which leaves `claude-agent-acp` in its own
# shipped default (`auto`), where Claude Code's classifier answers the easy
# ones. That is Anthropic's default for a reason and it is what makes the agent
# feel like every other harness.
#
# **This is thesis-compliant, and the distinction is worth being precise about.**
# The fork's thesis is about where data goes and whose credentials pay: no
# telemetry, no account, the user's own subscription. `auto` changes none of
# that -- the classifier runs inside the agent process on this machine, on your
# subscription, and `egress.rs` is unmoved. What `auto` costs is *Warp's
# visibility*: the event log records zero `permission_request` lines, because
# Warp is never asked. That is a measurement loss, not a safety loss, and it is
# what T14.18 named. The remedy T14.18 chose was disclosure, not compulsion --
# the panel reports the mode in force. So: run `default` when you want Warp in
# the loop and are prepared to answer for it. Run this the rest of the time.
set -u
cd "$(cd "$(dirname "$0")/.." && pwd)"

# Installed from `.fork/agents/claude-agent-acp.toml` and started by absolute
# path, never fetched through npx at launch
# (`.fork/decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`).
# This was `npx -y @agentclientprotocol/claude-agent-acp@0.73.0` until
# 2026-09-14; that pinned the adapter and left its dependency ranges free.
AGENT=$(python3 .fork/tools/agents.py path claude-agent-acp) || exit 1
if [ ! -x "$AGENT" ]; then
  echo "launch.sh: the agent is not installed at $AGENT. Install it with:" >&2
  echo "  python3 .fork/tools/agents.py fetch claude-agent-acp" >&2
  echo "  python3 .fork/tools/agents.py install claude-agent-acp" >&2
  exit 1
fi
export WARP_FORK_ACP_COMMAND="$AGENT"

# Instruments, off. Turn one on for the session where you need it:
#   WARP_FORK_EVENT_LOG=on     -- one JSONL per conversation
#   WARP_FORK_TRANSCRIPT=on    -- the conversation on disk, for grepping back
#                                 what compaction dropped (writes your prompts
#                                 to .warp/transcripts under the pane's cwd)
#   WARP_FORK_FRAME_LOG=on     -- slow-frame accounting

# WSLg: X11 rather than Wayland, which screenshots and global hotkeys need.
exec env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 \
  ./target/release/warp-oss "$@"
