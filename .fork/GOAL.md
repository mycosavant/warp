# The 24-hour goal: make the fork self-hosting

**Set 2026-08-29 for the weekend. Target: Monday morning 2026-09-01.**
Delete this file when it is met or abandoned — it is a horizon, not doctrine.

## What "done" means, as a run rather than a diff

> In **Warp's own agent panel**, in `/home/effatha/git/warp`, hold a
> **multi-turn** conversation that makes a real change to the fork, asks
> permission for it, is answered from the panel or from `warpctrl`, and whose
> **next turn remembers what it did**.

That is the whole thing. Everything below exists to reach that sentence. The
point is to stop driving this fork from Claude Code and start driving it from
the fork.

## Phase 0 — measure, and do it before building anything

**This has never been done, and the plan is worthless until it has.** Two agent
paths serve the panel, and on paper each has exactly the other's blocker:

| | multi-turn | interactive approval |
|---|---|---|
| `local_agent` (the `claude` CLI) | ✓ `--resume`, `mod.rs:353` | ✗ never emits a `ToolCall`, so `approvals.rs` has no branch to reach |
| `acp_agent` (any agent) | ✗ refused by `CANNOT_CONTINUE` | ✓ built on T14.6 |

That table is **read, not run**. So: drive a genuine multi-turn development task
through each path — not a one-line prompt, an actual small change to this repo —
and record what breaks. Two specific unknowns worth naming, because a guess
about either would set the next twenty hours off in the wrong direction:

- `local_agent` may already be closer than the table suggests. `claude -p`
  inherits the user's own settings, which are `defaultMode: auto` with 87 allow
  rules, so most tools may simply run. **What happens when one falls outside
  those rules is unmeasured**: `-p` has no TTY to prompt on, and the panel has
  no approval path, so the failure could be silent.
- `acp_agent`'s refusal is certain, but whether *anything else* is also missing
  for real work is not.

Write the answer into T14.7's as-built before writing code.

## Phase 1 — close the gap that Phase 0 names

Most likely one of these; the measurement decides, not this file.

- **`session/load` for ACP**, gated on the agent's advertised `loadSession`
  (`opencode` advertises it). The real design question is *what history it
  replays* and whether Warp then draws the transcript twice. Keep the honest
  refusal for agents that do not advertise it.
- **an approval path for the `local_agent` panel**, if Phase 0 shows that is
  the shorter road.

## Phase 2 — dogfood

Use the panel to make a real change to the fork, and commit it from there. Until
that has happened the goal is not met, however good the code looks.

## Stretch, only after 1 and 2 land and verify

- `AppendToMessageContent` token streaming — its `FieldMask` path must be
  **established by running it**, never guessed; nothing in this repo uses it.
- Diff rendering. The content is already there: an `opencode` edit request
  carries a full unified diff in `rawInput`, measured on T14.6. This is a
  legibility job, not a consent one.

## Guardrails

- Commit each increment on `dev`, findings in the body. **No push, no PR, no
  upstream merge** (T10 is a different kind of risk and is not this goal).
- `CARGO_BUILD_JOBS=8` on every release build — uncapped takes the WSL VM down.
- Leave no Warp or agent processes running; stop with `warpctrl window close`.
- Scratch profiles via `XDG_CONFIG_HOME`/`XDG_STATE_HOME` only. **Never** touch
  `~/.claude/settings.json` or the user's `settings.toml`.
- Verify by running. Name the inputs that were not verified.
