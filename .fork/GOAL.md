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

1. ~~**A requested mode may not survive `session/load`.**~~ **ANSWERED, run 1 —
   it does not survive, on every turn.** `session_mode`, written from the agent's
   own reply, read `current auto` on all four turns of one conversation, each
   after Warp had set `default` the turn before, while the agent raised a
   permission request on every one — which `auto` does not do. So the session
   returns from every resume in the agent's own mode and `mod.rs:660`'s
   unconditional re-send is the only thing that puts it back.

   **And this bullet's own framing was wrong.** It said the re-send happens
   *"because it is untested"*. The re-send has always been pinned by
   `a_repeated_request_is_still_sent_while_the_note_goes_quiet`, and
   `Decision::of`'s comment already reasoned that *"a resumed session may have
   come back in a different mode than it left"*. The design was right, documented
   and tested; what was unmeasured was whether the hazard is real. It is, every
   turn. Measurement now recorded at the call site, where someone would stand
   while deciding the line is redundant.

   The disguised-zero hazard named below is unchanged and is why the comment is
   there: **that zero is T14.17's falsifier wearing a disguise** — a reader sees
   "nothing needed asking" where the truth is "Warp was not in the loop".

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

## Run 1 — 2026-09-01, and what it left

**Log: `.fork/run-2026-09-01/friction.md`.** Seven turns against
`claude-agent-acp` 0.70.0 in `default`, both instruments on, on a binary rebuilt
at run start.

| | |
|---|---|
| refusal on the wire | **observed** — `reject_once`, request recorded verbatim, agent survived it |
| unknown 1 | **answered** — the mode does not survive a load; the re-send is load-bearing |
| unknown 2 | **cannot arise with this agent** — it serializes; two were never parked at once |
| unknown 3 | **not observed** — 4/4 turns closed, permission events balanced 8/8 |
| unknown 4 | **not reached** — seven turns did not come near compacting |

**So this horizon is not met, and the gap is precise**: one long session, for
unknown 4 and for the eight-hours caveat. The refusal criterion is satisfied and
does not need repeating. Three of the four are done, and #2's answer is a fact
about the agent rather than about the fork, so it stays a *"not observed"* rather
than a *"does not exist"*.

Fixed after the run closed, both earned by it: the mode note now reports the
agent's answer instead of hedging about a question settled two lines earlier, and
`NOTHING_IS_WAITING` — a review fix that added a constant while leaving the
renderer's duplicate in place — actually has one home now. Neither is a
permission-posture change.

## Between run 1 and run 2 — 2026-09-02, the platform was the blocker

**Run 2 could not have happened on the machine this fork is actually used on,
and nobody knew.** Every turn in every WSL pane on the Windows build died at
`session/new` — *"`cwd` does not exist on the machine running the agent"* — because
the shell reports a Linux path and Warp had started the agent on Windows. Fixed
in T18 by starting the agent inside the distribution, which is the treatment
`local_agent` has given `claude` since T6.1.

**This matters to the horizon and not just to the board.** Run 1 was seven turns
on the Linux build, where agent and shell share a filesystem and the bug cannot
appear. The horizon asks for a long session *doing work someone actually wanted
done*, and the work is on Windows with WSL panes — so the destination was
unreachable on the real platform while looking fine on the tested one. That is
worth naming as a class: **a horizon measured only where the bug cannot occur is
not measured.**

Also cleared on the way, none of it a permission-posture change: the WSL agent
path and its gotchas are now in `.fork/README.md` rather than in one session's
memory; the Windows build's second checkout is documented as something nothing
syncs; and T17 answered "should Warp give agents LSP" with *no* — the recommended
pairing already has the whole surface, measured, so that question is closed
without code.

## Run 2 — the remaining gap, unchanged and now reachable

Three of the four unknowns are answered and the refusal criterion is met. **What
is left is one long session, on Windows, in a WSL pane**, for unknown 4 and for
the eight-hours caveat run 1 carried forward.

Nothing new needs building for it. The instruments are on
(`WARP_FORK_EVENT_LOG`, `WARP_FORK_TRANSCRIPT`), the agent pairing is settled
(`claude-agent-acp` + `WARP_FORK_ACP_MODE=default`), and as of T18 the bare
command works in a WSL pane with no manual wrapping. The one thing to resist is
the one this file already names: **building the compaction detector before
measuring the cadence.**

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

- ~~**`PAIRABLE_ACTIONS` has no test pinning its membership.**~~ **Retracted
  2026-09-01 by calibration; this file kept the wrong version until 2026-09-02.**
  It is pinned by two tests in `app/src/local_control/pairing_tests.rs`:
  `a_paired_device_gets_the_read_surface_and_the_safe_half_of_answering:141`
  asserts the **whole list** against a literal slice — membership, not a count,
  so it is strictly stronger than the catalog pin it was being compared
  unfavourably against — and widening the list also reddens
  `saying_yes_does_not_travel_by_default_and_saying_no_does:169`, which holds the
  consent asymmetry. **Re-calibrated 2026-09-02 rather than taken from
  `CLAUDE.md`**, because this correction is what the next run starts from:
  adding `ActionKind::AgentApprove` to the list turns `local_control::pairing`
  from 10 passed to 8 passed and reddens exactly those two. Scope named — that
  filter, not the workspace; `pairing_tests.rs` holds the only references to the
  list, so a wider sweep was not run.

  **What did go stale is the count in `CLAUDE.md`, and no test can pin prose** —
  which is the whole reason that file tells you to read a number off the test.
  The observation was right and the mechanism invented under it was not, the same
  shape as the discovery-record retraction. So there is **no ticket to raise
  here**, and this entry stood one edit away from funding a panel task to build a
  test that already existed.
- **T14.21 exists in git with no ticket in `TASKS.md`.**
- **Seven `- [ ]` boxes in `TASKS.md` are stale**: the work landed. Read status
  off the as-built records, not the checkboxes.
- **Done 2026-09-01.** `CLAUDE.md:1057` called the `warp-tui-oss` spinner
  unverified and floated a model-credential hypothesis; I20 had since measured
  it as a device-code OAuth account gate. Corrected in `CLAUDE.md`.
