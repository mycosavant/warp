#!/usr/bin/env bash
# Dump a Claude Code hook's stdin verbatim, then get out of the way.
#
# Why this exists (T14.20). The fork wants to tie an approval to the edit it
# produced, which needs a per-call id. `TR-EVENTS-B` is named in three files as
# "would have to come from the plugin" -- but the `warp` plugin's own
# `on-permission-request.sh` reads stdin, extracts `tool_name` and `tool_input`,
# and never looks for an id. So the plugin not carrying one is not evidence that
# the payload lacks one. This keeps the bytes and settles it by measurement.
#
# Install (do not let anything here edit settings for you -- add it yourself):
#
#   "PermissionRequest": [{ "hooks": [
#     { "type": "command", "command": "/home/effatha/git/warp/.fork/tools/dump-hook-stdin.sh permission-request" }
#   ]}]
#
# Read the result with:
#   jq 'keys' ~/.local/state/warp-oss/hook-dumps/*-permission-request.json
#
# Override the destination with WARP_FORK_HOOK_DUMP_DIR.

set -uo pipefail

dir="${WARP_FORK_HOOK_DUMP_DIR:-$HOME/.local/state/warp-oss/hook-dumps}"
event="${1:-hook}"

# A diagnostic must never change what the agent is allowed to do. A
# PermissionRequest hook that exits non-zero can deny the very call it was only
# meant to watch, so every failure path below still ends in `exit 0` -- and the
# stdin is drained either way, so nothing blocks on an unread pipe.
if ! mkdir -p "$dir" 2>/dev/null; then
    cat >/dev/null 2>&1 || true
    exit 0
fi

out="$dir/$(date -u +%Y%m%dT%H%M%SZ)-$$-${event}.json"
cat > "$out" 2>/dev/null || true
exit 0
