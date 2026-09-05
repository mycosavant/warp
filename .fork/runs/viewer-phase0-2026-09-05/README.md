# Viewer phase 0, measured (board item 6)

Windows debug build `v0.fork.7bb323cbc`, the user's own profile (not a
scratch one), launched through `warpdev.ps1 -EventLog`: the product profile
plus `WARP_FORK_EVENT_LOG=on` and nothing else. `claude-agent-acp@0.73.0`
started inside WSL, no `WARP_FORK_ACP_MODE`, so the session ran in the
agent's own `auto`. One panel turn asking for two tool calls, a `Read` and a
`Bash`. Driver: `phase0.sh`, output in `driver.log`.

The question phase 0 asked: does the join between Warp's event log and the
agent's own session file hold on a live product-profile session, where Warp
is never asked? **Yes.**

| | |
|---|---|
| `session_mode` | `current auto` |
| `permission_request` lines | 0 |
| `tool_start` / `tool_complete` | 2 / 2, `call_id` = `toolu_…` |
| `linked_session_id` on every tool line | `dc585483-6b63-4ded-9b07-f58c85f28502` |
| the harness's file | `~/.claude/projects/-home-effatha-git-warp/dc585483-….jsonl`, 24 lines, Claude Code 2.1.257 |
| both `call_id`s in that file | 3 lines each: `tool_use` in an `assistant` line, `tool_result` in a `user` line, one `attachment` |
| turn wall time | 9 s from prompt to `is_busy: false` |
| second instrument | `C:\dev\shots\phase0-ebacd6e3-445c-41db-8edd-5b6f5b781b8c.png`: the panel shows both tool rows and the answer quoting commit `7bb323cbc` |

Two things the run showed that the observability page had not said:

- **On the ACP path Warp's log carries no tool input at all**, not a
  truncated one: `tool_name` is the ACP kind (`read`, `execute`) and
  `tool_input_preview` is `None`. The harness file has the real name and the
  full input (`{"file_path": "/home/effatha/git/warp/CLAUDE.md", "limit": 1}`).
  So the frame is stronger than run 2 suggested: for this path the harness
  file is not a fuller copy of the log, it is the only record of what ran.
- **The first line of the session file has no `cwd`, `version` or
  `gitBranch`**; it is one of the bookkeeping kinds
  (`operation`, `sessionId`, `timestamp`, `type`). The viewer must take those
  from the first line that has them, not from line one.

The slug rule, measured with one `claude -p` turn from a directory named
`slug probe/a_b.c` (`slug-probe.diff`): `/`, space, `_` and `.` all become
`-`; letters keep their case; `-` stays. So the slug is lossy and cannot be
inverted. The viewer computes it from the `cwd` Warp's log already carries,
as `[^A-Za-z0-9]` → `-`, and looks for `<linked_session_id>.jsonl` there.
Non-ASCII letters were not tried.

Not measured here: the `local_agent` path live. Its join key was written by
the same commit and is pinned by unit tests; the product profile runs the
ACP agent, so this run exercises the ACP half.

Two driver notes. Launching through `warpdev.ps1` with stdout piped to `tee`
hangs the caller: Warp inherits the pipe and the launcher's exit never
closes it. Run the launcher with its output going to a file or a terminal.
And `session inspect` answered `ambiguous_target` because the restored
layout had two tabs; the prompt went to the active pane, which the restored
session had left in this repo, so the `cd` was moot and the ambiguity did
not matter for the measurement.
