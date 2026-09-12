# End-to-end park-and-approve inside a TUI session, driven live

Closes the item that has stood open across three handoffs (`HANDOFF-DECIDE.md`
→ `HANDOFF-NEXT.md` → `HANDOFF-GHTESTS.md`) and in `.fork/docs/agent-transports.md`
since f2e1558c6 (2026-09-11) wired `warpctrl` into the TUI: *"The channel is
proved; that loop is not."* The loop is now proved too.

## Setup

Scratch profile, no shared state with the maintainer's real config:

```
SCRATCH=.../scratchpad/tui-approve-test
mkdir -p "$SCRATCH"/{config,state,workdir}
cargo build -p warp_tui --bin warp-tui-oss --features standalone,warp_control_cli

cd "$SCRATCH/workdir"
XDG_CONFIG_HOME="$SCRATCH/config" XDG_STATE_HOME="$SCRATCH/state" \
WARP_FORK_ACP_COMMAND="npx -y @agentclientprotocol/claude-agent-acp@0.73.0" \
WARP_FORK_ACP_MODE=default \
tmux new-session -d -s tuicheck -x 120 -y 40 './target/debug/warp-tui-oss'
```

`WARP_FORK_ACP_MODE=default` is required — without it `claude-agent-acp`'s own
`auto` classifier answers permission requests before Warp ever sees one,
exactly as documented for the GUI pairing. Confirmed in the transcript: *"This
turn began with the session in the agent's auto mode... `default` mode, which
the agent describes as 'Always ask before making changes', and the agent
accepted."*

## The drive

1. From inside the tmux pane: `Write the text hello-from-tui-test to a file
   called out.txt in the current directory.`
2. The turn parked. The TUI's own transcript named the exact remedy, including
   the approval id and digest requirement, matching `asking_note`'s post-f2e1558c6
   text verbatim:

   > The agent is waiting for permission: Write out.txt
   >
   > Answer yes with `warpctrl agent approve 76bdc904-c74d-43e9-b773-91ed4b18f9bc:0`
   > or no with `warpctrl agent deny 76bdc904-c74d-43e9-b773-91ed4b18f9bc:0` —
   > both take the digest that `warpctrl agent approvals` reports.

3. From a **separate shell**, using `warp-oss`'s `--warpctrl` client (the
   standalone `warp-tui-oss` binary's own CLI has no `--warpctrl` flag — it
   only answers `dump-settings-schema`; the client that talks to a running
   instance over local control is the one built into `warp-oss`, and the
   instance is discoverable there regardless of which binary is serving it):

   ```
   warp-oss --warpctrl instance list --output-format json
   #  -> app_id: "dev.warp.WarpTui", confirming this is the TUI's own record

   warp-oss --warpctrl agent approvals --instance <id> --output-format json
   #  -> approval_id, digest, tool_name: "edit", tool_input naming out.txt's
   #     full path and exact content

   warp-oss --warpctrl agent approve <approval_id> --digest <digest> --instance <id>
   #  -> {"ok": true, "decision": "allow", "keystroke": "allow-once"}
   ```

## What it proved

- The TUI's transcript resolved and the turn proceeded without any input to
  the TUI pane itself.
- `warpctrl agent approvals` against the same instance immediately after
  returned `"approvals": []` — the request cleared.
- **The file was actually written**, read back from disk after the approval,
  not inferred from a status line: `out.txt` contains exactly
  `hello-from-tui-test`.

This is the strongest evidence available short of instrumenting the ACP wire
directly — a status message can lie or stall; a file's content on disk cannot.

## What this does not establish

- Only the **approve** path was driven live. **Deny** goes through the same
  `registry::answer(id, Allow/Deny, ...)` call (confirmed by reading
  `app/src/local_control/handlers/approvals.rs:197`) but was not exercised
  end-to-end in this run.
- Only `claude-agent-acp` was tested. `codex-acp` and `opencode` were not
  re-verified against the TUI specifically (though nothing in the TUI wiring
  is agent-specific — the whole mechanism lives in `app/src/ai/acp_agent/`,
  shared with the GUI panel).
- Not tested: a paired remote device (`WARP_FORK_REMOTE_APPROVE`) answering a
  TUI-parked request. The note's own text says a paired device's "yes" only
  travels when that variable is set; this run answered from a local shell.
- Single permission request, single tool call (`edit`/write). A tool call
  needing a *choice* among multiple non-yes/no options was not exercised.

## Cleanup

`tmux kill-session -t tuicheck`; the TUI process, `warp-oss --warpctrl instance
list` confirmed empty afterward; no lingering processes (`pgrep -x
warp-tui-oss node warp-oss` all empty). Scratch profile left on disk under the
session's own scratchpad directory, not under the maintainer's real config.
