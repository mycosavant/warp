# The horizon: the consent path, seen on the wire

**Set 2026-09-01, replacing the horizon set 2026-08-30** (*"a daily driver by
Monday"*). Delete this file when it is met or abandoned — it is a horizon, not
doctrine. **Read it first; it outranks `TASKS.md`'s ordering while it stands.**

---

## First, the verdict the last horizon never got

Its criterion was *"a day's work has been done through it and the friction log
contains nothing that stops a turn"*. **Met as written**, on evidence committed
in `a27ffaff4` and `.fork/friction-2026-08-31-clean.md`: 15 turns, 0 permission
requests, 0 turns stopped, `mode: default` throughout, `permissions_denied`
absent on all 37 conversations. Both stops in the earlier log were traced by
running to causes outside the fork's consent design, and that tracing is what
changed the recommended agent.

No commit body or doc recorded that verdict for a day, which is why it is written
here rather than assumed. **And the caveat that file insisted on is carried
forward rather than absorbed:**

> Not claimed: eight continuous hours. Thirty turns across a session that crossed
> midnight is a day's work by output, and nothing here establishes how the fork
> feels at that length.

The clean run does not close it. Every turn in it is a short scoped audit, 38 to
199 seconds. That is the thread this horizon picks up, but it is **not** the
destination — length is the setting, not the goal.

---

## Destination

> **Watch the fork refuse something, on the wire, in a session long enough for
> the things that only appear at length to appear.** Met when the refusal path
> has been observed rather than argued, and when a long session has either
> produced the four failures named below or shown, with instrumentation running,
> that they did not occur.

The measurement is the deliverable. **Build nothing first.** This fork's most
repeated finding is that the thing was already there, and its second most
repeated is that a measurement changes what is worth building — the sixteen-ask
storm that looked like the case for I18 evaporated when the question was re-asked
with a boundary, the same day.

## Why this and not a feature

`TASKS.md:6906`, in the fork's own words:

> **the *allow* path is measured and the *refusal* path is argued.**

Every refusal in `acp_permission.rs` is pinned by a test, and **none has been seen
on the wire**, because `opencode` never sends a `switch_mode`. The fixtures are
real — transcribed field for field from what two agents actually sent — but a
transcribed *option list* is not a *decision observed*. The fork's entire thesis
is consent, and the half of consent that says **no** has never been exercised by
an agent trying to do the thing.

That is a bigger hole than any unbuilt ticket on the board, and it costs a run
rather than a build.

## The four things that only appear at length

Each is already recorded as unverified; the point of naming them together is that
**a single long run with instrumentation is the only place any of them can
show up.** They are not four projects.

1. **A requested mode may not survive `session/load`.** (`TASKS.md:9270`)
   Untested — the mode is re-sent every turn *because* it is untested. If it does
   not survive a resume, the session silently reverts to `auto`, the agent's own
   classifier answers, and the event log records **zero** permission requests.
   **That zero is T14.17's falsifier wearing a disguise**: a reader sees "nothing
   needed asking" where the truth is "Warp was not in the loop".

2. **Two requests parked on one connection.** (`TASKS.md:6714`) Never tested —
   both agents measured so far were separate processes, so a blocked dispatch
   loop would not have shown. A real session raising two concurrent asks is
   ordinary, and a deadlock would present as a wedge, which is the failure this
   fork has the least ability to tell apart from an agent thinking.

3. **A turn that vanishes.** (`TASKS.md:9270`) Seen twice in four turns: no
   output, no note, no answer, a `session_start` with no `stop`. Predates
   T14.18, is not the mode path, *"recorded rather than explained"*. A turn that
   disappears silently is the exact failure class the event taxonomy exists to
   catch, and it is currently uncaught.

4. **Compaction under ordinary work.** (`TASKS.md:8175`, `:8401`) The
   "every five or six turns" figure is **retracted** — it came from a
   deliberately context-hostile payload. The honest statement is that the cadence
   under ordinary work is unmeasured, and the disclosure half is unbuilt: the
   panel cannot say the agent's view and Warp's have diverged.

## What "met" looks like, concretely

- A run against **`claude-agent-acp` in plan mode**, long enough to compact at
  least once, doing work someone actually wanted done.
- `WARP_FORK_EVENT_LOG` and `WARP_FORK_TRANSCRIPT` on for the whole run, and read
  **in timestamp order** — the log files per conversation, so `cat *.jsonl` gives
  filename order and a reader inferring causality from line order will be wrong.
- **At least one refusal observed on the wire**, with the request that triggered
  it recorded verbatim, not transcribed from memory.
- A friction log that says, for each of the four, either what happened or that it
  did not happen with the instrument running. **"Not observed" is a result here
  and "not looked for" is not.**

## What would make this fail, stated so it can

- **Finding nothing and calling that success.** If the run produces no refusal,
  the horizon is not met — it is un-run. A session that never trips the boundary
  has not tested it.
- **Fixing frictions mid-run.** The last horizon's own sentence applies unchanged:
  frictions that merely annoy are the success condition, not a failure.
- **Building any of the four remedies before measuring.** Especially the
  compaction detector, which is scoped and tempting and would be built against an
  unmeasured cadence.

## Not this horizon, and why

- **I18, the persistent grant.** Frozen, and its measured case was retracted the
  same day it was made. Permission posture stays frozen; that decision is the
  maintainer's.
- **The I20 TUI answerer.** Deferred on a stronger argument than sequencing: a
  TUI session carries the fork's *telemetry* posture and **none** of its consent
  posture, so it would look like the same product and be something else.
- **T14.14's picker.** The seam is built and tested (`model.rs`); what remains is
  UI, which no friction log has asked for.
- **T14.20's increments.** The recovered call id is heuristic, and the ticket's
  own ruling stands: *a call id that is usually right is worse than none for an
  audit trail.*
- **I21's viewer.** Cheap and unbuilt, but it serves a workflow complaint rather
  than a claim about the fork's behaviour.

## Standing constraints

No push, no PR, no upstream merge without an explicit say-so. **Permission
posture stays frozen.** `CARGO_BUILD_JOBS=8` — and note the cap was chosen
against a 32 GB VM that no longer exists (`CLAUDE.md:902`), so re-measure before
re-tuning rather than raising it on the assumption that headroom exists. Leave no
Warp or agent processes running.

## Board hygiene this replaces, done or noted

- **`PAIRABLE_ACTIONS` has no test pinning its membership**, which is why its
  count went stale in `CLAUDE.md` for two days while the 114-action catalog —
  pinned in two crates — stayed right. Adding that pin changes the consent
  surface's guardrails and wants a ticket.
- **T14.21 exists in git with no ticket in `TASKS.md`.**
- **Seven `- [ ]` boxes in `TASKS.md` are stale**: the work landed. Read status
  off the as-built records, not the checkboxes.
- **`CLAUDE.md:1057` still calls the `warp-tui-oss` spinner unverified** and
  floats a model-credential hypothesis; I20 has since measured it as a
  device-code OAuth account gate. Same fact, two files, one stale.
