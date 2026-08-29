# The horizon: use the fork to build the fork, as the default

**Set 2026-08-29. Supersedes the 24-hour self-hosting goal, which was met the
same day.** Delete this file when it is met or abandoned — it is a horizon, not
doctrine.

---

## Where this starts

The previous goal — *one* multi-turn conversation in Warp's own panel that
changed the fork, asked permission, was answered from `warpctrl`, and remembered
itself on the next turn — was met by commit `cddacfbc7`. The account is T14.7's
as-built in `TASKS.md`.

That proved it is **possible**. This goal is about it becoming what actually
happens, which is a different and less glamorous problem.

## Destination

> **A full working session on the fork, driven entirely from the fork.** Not a
> demonstration: a real item off this board, taken end to end in the panel, long
> enough to meet what only length reveals — and with consent answered by
> *pressing* something rather than by copying two seventy-character strings.

Met when a commit lands that was produced that way and the session's own
friction log is short enough that a person would choose to do it again.

## The order, and why this order

**T14.9 first: run the long session before building anything for it.** This is
the whole lesson of T14.7 Phase 0, where the two blockers everyone predicted
were both wrong and the real one — the pane starting in `$HOME` — was in neither
cell of the table. Every candidate below is a guess about which friction
dominates until a real session ranks them.

**T14.8 second: build whatever T14.9 puts at the top.** The current favourite is
an in-panel approval control, and it is a well-founded favourite — seven
permission requests in three turns, each answered by copying an id and a digest.
But whether the answer is a button, a cheaper way to *address* the pending
request from a shell, or something the session surfaces that nobody has thought
of, is exactly what a real run decides.

## Candidates, ranked by present evidence rather than by conviction

- **No button.** Consent is answered by typing `warpctrl agent approve <id>
  --digest <d>`. Doable from Warp, because Warp is a terminal. Not comfortable.
- **No cheap addressing.** There is no `--latest`. The id and digest are opaque
  and long, and both must be copied exactly. This may capture most of the
  friction for a fraction of the button's cost, which is why it is listed
  *against* the button and not beside it.
- **The event log is blind on this path.** `CLAUDE.md` says to reach for
  `WARP_FORK_EVENT_LOG` before theorising about what an agent did. On the ACP
  path it writes `session_start` and `stop` and nothing between them, so the
  fork's own recommended instrument does not work on the path the fork now uses.
- **No `/compact`.** A day's work will outgrow a context window; `local_agent`
  handles compaction and the ACP path does not.
- **No model selection.** `session/load`'s reply carries `configOptions` with a
  model select, so the protocol already offers it.
- **A deterministic refusal is retried three times** with backoff before it is
  shown. Small, and now rare.

## Ruled out, with the measurement that ruled it out

**Replay cost at length is not a problem.** The worry was that spawning an agent
per turn and replaying the whole history through `session/load` would make long
conversations progressively slower. Measured 2026-08-29 over stdio, no Warp:
`session/load` took **0.34s at one exchange of history and 0.34s at six**, with
process startup (~0.7s) dominating. Replayed notifications grow exactly two per
exchange, which is linear and cheap. Unverified: the same at fifty exchanges, and
with large tool outputs in the history rather than short text.

## Not this weekend

T10's upstream merge and I18's OpenRouter provider are real and are not this.
Merging upstream while the agent surface is changing under it would confuse two
kinds of breakage, and the merge cost is paid by deferral rather than by
divergence — so it is a deliberate wait, not an oversight.

## Guardrails

Unchanged from the goal this replaces, and they earned their place:

- Commit each increment on `dev`, findings in the body. **No push, no PR, no
  upstream merge.**
- `CARGO_BUILD_JOBS=8` on every release build — uncapped takes the WSL VM down.
- Leave no Warp or agent processes running; stop with `warpctrl window close`.
- Scratch profiles via `XDG_CONFIG_HOME`/`XDG_STATE_HOME` only. **Never** touch
  `~/.claude/settings.json` or the user's `settings.toml`.
- **`cd` the pane into the repo before the first prompt.** A fresh pane starts in
  `$HOME`, both agent paths take the session cwd from the pane, and for an ACP
  agent that directory also decides whether its permission rules load at all.
- Verify by running. Name the inputs that were not verified.
