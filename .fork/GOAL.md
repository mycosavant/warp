# The horizon: a daily driver by Monday

**Set 2026-08-30, replacing the horizon set 2026-08-29** (*"the fork builds the
fork, and keeps going when nobody is watching"*), which was met in its build half
and spent in its ordering half. Delete this file when it is met or abandoned — it
is a horizon, not doctrine. **Read it first; it outranks `TASKS.md`'s ordering
while it stands.**

---

## Destination

> **A working day on the fork, on real work, driven from the fork.** Not a
> demonstration and not one conversation. Met when a day's work has been done
> through it *and* the friction log contains nothing that **stops** a turn.

Frictions that merely annoy are the **success condition, not a failure**. That
is the whole difference between this horizon and the one before it, and it is
the sentence to re-read whenever a run produces a list of complaints and the
temptation is to fix them before continuing.

## Where this starts

The previous horizon asked for a multi-turn conversation in Warp's own panel that
changed the fork, asked permission, was answered, and remembered itself. That was
met. Everything since has been filing the sharp edges one real session found —
T14.8's unanswerable requests, T14.10's silent turns, T14.13's invisible
compactions, T14.16's in-panel button, T14.19's transcript recovery.

**What still has not happened is a second real session.** Two rounds of fixes
have landed on a measurement nobody has re-taken, and running first has
overturned the plan three times in T14 — Phase 0's table was wrong in both cells,
T14.9 demoted the button everyone was sure about, and T14.12 was closed without
being built because `agent read` turned out to stream after all. The prior is
strong and it is cheap to honour.

## The three blockers, and where each stands

1. **T14.17 — the ACP path logged what the agent did and never what it asked.**
   **Done 2026-08-31** (`81b9b334a`). `permission_request` and
   `permission_replied` now carry the `tool_input` that was shown, what was
   decided (`allowed`/`denied`/`unanswered`), which surface answered, and whether
   Warp had a *yes* to offer at all. Observation only: it changes no approval
   outcome, and a test pins that logging emits no client action.

2. **I18 — the persistent grant. Documented, deliberately unbuilt.** The
   evidence arrived (`permission_suggestions` is a first-class session-scoped
   grant Claude Code already offers and the fork drops), and with it a finding
   that decides the shape: **the two transports fail at opposite ends.** ACP can
   *answer* an `allow_always` but cannot *describe* it; the Claude Code hook can
   describe one precisely but cannot answer at all. So `acp_permission`'s rule —
   *an option may only be selected by a surface capable of showing what it
   declares* — is correctly refusing the only path that can act. **Permission
   posture stays frozen**; this is the largest posture change on the board and
   the decision is the maintainer's.

3. **I20 — the TUI answerer. Deferred, not blocked.** The gate is cosmetic for
   fork transports and the seam answers; the cheap path is one `LaunchMode` arm
   plus a cargo feature, which would also serve the T12 console and its existing
   per-entry gating with zero new consent-surface code. **Build nothing a
   friction log has not asked for** — the panel earned its button after 35
   measured copy-paste approvals; the TUI has none. Named hazard: **type-ahead**,
   because an Enter already in the buffer when a TUI prompt takes focus is a yes
   nobody gave.

## Run 2026-08-31: what it took, and what it cost

**Two working sessions, 30 turns, and the two friction logs disagree — which is
the result rather than a problem with it.**

| log | agent | turns | asks | **stopped** |
|---|---|---|---|---|
| `.fork/friction-2026-08-31.md` | `opencode acp` | 21 | 22 | **2** |
| `.fork/friction-2026-08-31-clean.md` | `claude-agent-acp` + `WARP_FORK_ACP_MODE=default` | 7 | 0 | **0** |

**Both stops in the first log were traced, by running, to causes outside the
fork's consent design**, and the tracing is what changed the recommendation:

- **An unscoped question.** *"Trace where the warning goes and who sees it"* sent
  the agent across three subsystems and then outside the project. Re-asked with
  *"answer only from these two files"*: 0 asks, 50 seconds, complete answer. Six
  scoped audits later, still 0–1 asks each.
- **`opencode` abandons a turn after a refusal.** Same prompt, same refusal,
  measured against both agents: `opencode` emitted nothing further even when told
  what to do instead; `claude-agent-acp` in `default` mode answered around it.
  **Warp sends the identical per-call rejection in both cases** — never
  `Cancelled` — so the difference is entirely the agent's.

**So the fork's documented recommendation changed on the strength of that**, and
that is the honest resolution of the two logs: they are not two readings of the
same thing, they are the measured-inferior configuration and the recommended one.
CLAUDE.md now carries the table and names the pairing, because either half of it
alone is worse than `opencode`.

**What the run produced**, which is the part worth keeping: 14 defects fixed and
9 docs corrected, including a `serde_json/preserve_order` regression in which this
fork's own ACP dependency silently broke this fork's own git-backed Warp Drive
sync — no line of `local_sync` changed, no gate in the repo catches it, and the
tripwire test that names the hazard had been red for an unknown period because
nobody ran it. Five more were one class: **a doc that outlived its code.**

**Not claimed:** eight continuous hours. Thirty turns across a session that
crossed midnight is a day's work by output, and nothing here establishes how the
fork feels at that length — every friction logged came from a short burst.

## So the remaining work is to run it

**T14.11, and it is now person-present by construction.** Unattended, all three
doors to a panel-made commit are shut — the harness refusal, the flagship agent
(whose `auto` mode never asks Warp and whose `default` mode wedges with no
answerer), and the permission freeze. That is the consent design working, not a
defect. T14.16's button is where the person answers.

**And there is something new to measure while running.** T14.17's falsifier is
live and costs nothing extra: with `WARP_FORK_EVENT_LOG=on`, if
`permission_request` count is **zero while edits land**, the instrument recorded
only Warp's non-involvement — which is exactly what T14.18 predicts for
`claude-agent-acp` in `auto` on the panel path, since no `session/set_mode` is
ever sent. In that case the mode surface should have led and T14.17 measured
nothing. A dogfood needs something to be building while it gets long, and it
needs something to be watching; this is the second.

**Check the binary's timestamp before believing any run.** A release build
started before a fix and run after it measures the pre-fix binary — that cost a
rebuild and a wrong conclusion on 2026-08-30. `date -r target/release/warp-oss`
against the newest file touched settles it in one second.

**Reorder this list if a run says to.** That is what it is for.

## The charter

**Written for unattended work, and it does not relax when someone is watching.**
It was set down 2026-08-29 because the usual backstop — someone reading the next
message — was missing. The maintainer is present again for this horizon, and the
rules stand unchanged, because a rule that only holds when nobody is looking was
never a rule.

**The one that matters most under a deadline is the permission freeze.** A
horizon named *"daily driver by Monday"* is exactly the pressure that makes a
one-line widening look like progress, and the freeze exists for that moment
rather than for the calm ones. Its own words: *one approved line is consent for
that line; it is not a pattern to continue.* A blanket go-ahead is not consent
for a posture change — ask for that one specifically, every time.

> **Known hole, found by trying it 2026-08-29.** The charter treats *answering
> the agent's permission prompts* as an allowance to be governed by rules. It is
> first a **capability**, and unattended it does not exist: starting an automated
> approval harness was refused by the orchestrating session's own permission
> layer, which is the right refusal — an unattended script answering an agent's
> prompts is the consent architecture with no person in it. So the destination
> below assumes an answerer that is not available. Until that is settled, an
> unattended run can do everything **except** drive an in-panel agent that asks
> for anything. See T14.11's as-built.

**Proceed without asking:** measuring and probing locally; editing fork-owned
files; tests; release builds **capped at `CARGO_BUILD_JOBS=8`**; committing each
increment on `dev` with the findings in the body; launching and stopping Warp
under scratch `XDG_CONFIG_HOME`/`XDG_STATE_HOME`; driving agents locally;
enlisting the advisor for decisions.

**Answering the in-panel agent's permission prompts is part of the job, and it
is the one allowance that needs rules.** Unattended, an orchestrator answering an
agent short-circuits the consent architecture: the digest binds the answer to
what was shown, but nothing was shown to a *person*. Two rules make it
defensible, and both are cheap.

1. **Approve only what the charter itself allows.** An edit inside the repo, a
   local read, a capped build — yes. Anything else, and **anything Warp marks
   `can_approve: false`, is denied.** Never route around a refusal: if Warp
   would not say yes, an orchestrator saying yes on its behalf is the whole
   safeguard defeated.
2. **Log every decision with the `tool_input` that was shown**, so each landed
   edit can be traced tomorrow to the answer that let it happen. T14.9 measured
   the failure this prevents: two polling loops answered at once and edits landed
   that the friction log never recorded.

   > **This rule had no implementation when it was written, and T14.17 landed
   > one on 2026-08-31.** What was found on 2026-08-30: the ACP translator wrote
   > `tool_start` and `tool_complete` and nothing else, and across every
   > recorded ACP session in this repo there was not one permission event,
   > because none was ever written. The ask and the answer were transcript
   > prose, read live and not kept — so an orchestrator obeying this rule had to
   > keep the record itself, and the trail was its word rather than the fork's
   > evidence.
   >
   > The ACP path now emits `permission_request` and `permission_replied` under
   > `WARP_FORK_EVENT_LOG`, carrying the `tool_input` that was shown, what was
   > decided (`allowed`/`denied`/`unanswered`), which surface answered, and —
   > on the ask — whether Warp had a *yes* to offer at all. So the rule above is
   > now satisfied by the fork rather than by the orchestrator's honesty.
   >
   > **Two things that have not changed.** This was always a gap in the audit
   > trail and never in the boundary: nothing was auto-approved before it and
   > nothing is now, Warp manufactures no yes, and the refusals fire either way.
   > And the log only records what Warp was *asked* — T14.18 measured a panel
   > session with `claude-agent-acp` producing **zero** permission requests,
   > because its own classifier answered first. A run with no lines in it is
   > therefore not evidence that nothing was decided; it is evidence Warp was
   > not in the loop, which is T14.17's own falsifier and the thing to check
   > before reading any count off this log.

Two more, from measured failures rather than from caution. **Single writer:**
while the panel session is editing the tree, the orchestrator does not. **Single
build:** never overlap an orchestrator build with a panel-triggered one — two
capped builds is sixteen effective jobs on a VM that died at thirty-two.

**Stop and write it up instead of doing it:**

- **push, PR, or upstream merge.** Standing, and unchanged.
- **any further change to permission posture, and this beats "edits to
  fork-owned files" wherever they meet.** They meet at exactly one place and it
  is worth naming, because an agent optimising for progress at 3am will find it:
  `opencode.json` **is** a fork-owned file, and the obvious fix for a permission
  stall is one line shaped exactly like the line the maintainer approved. The
  freeze wins. It covers `opencode.json`'s `permission` block, anything under
  `.claude/`, and both settings files. One approved line is consent for that
  line; it is not a pattern to continue.

  > **Exercised 2026-08-30 and it held.** The `bash` pattern map was measured,
  > documented, and left **unapplied** across two sessions while the maintainer
  > was away, then applied the moment they said so. That is the freeze working
  > as designed rather than an argument for relaxing it: the next
  > permission-shaped change starts frozen again, including further entries in
  > that same map.
- the user's `~/.claude/settings.json` or `settings.toml`; **anything outside
  this repo and the scratch directories.**
- installing packages or fetching from the network beyond what is already
  cached.
- deleting or rewriting history, and `kill` on a Warp that `window close` would
  stop — cancel the turn first.

**Stop conditions — put the work down and report:**

- gates red beyond the known-flaky set (`gh`-dependent git tests, secret
  redaction, terminal view). **Diff membership, never counts.**
- **a wedge, with a number now that there is one.** `quiet_for_seconds` past
  about ten minutes with **no** pending approval — `waiting_for_you` is the field
  that tells those apart — means `agent cancel`, log it, and resume once through
  `session/load`, which is measured to lose nothing. A second wedge on the same
  task is a write-up, not a third attempt. **Cancel before `window close`**, or
  it will not close.
- **context exhaustion is a finding, not a failure.** Nobody has seen what it
  looks like from the panel. Record it, then start a fresh session rather than
  fight it.
- **disk.** Scratch profiles, event logs and release artifacts accumulate over
  hours. Check free space before each build and stop below a floor rather than
  discovering it mid-link.
- **a measurement whose apparatus could have produced it.** Unattended, a false
  finding committed at 3am is worse than no finding, because it will be believed
  in the morning. The failure that actually happened: a polling loop passed the
  digest positionally, the turn parked, and it looked exactly like a wedge — with
  the instrument that distinguishes them sitting right there.

  **This is the merge-base lesson wearing a new coat.** There, `git diff A...B`
  ran perfectly and reported honestly about a base that had been assumed rather
  than computed. Here, `quiet_for_seconds` reported honestly downstream of an
  input that had been assumed: *that the approval was delivered*. Three tiers,
  cheapest first:

  1. **Read back every state-changing step before measuring what follows.** After
     `approve`, confirm the request has left `agent approvals` — or that the turn
     resumed — before believing any number about the turn. One extra read per
     action, run every time. It would have caught this in seconds, because the
     request was still sitting in the list.
  2. **Calibrate a new instrument against a known answer before trusting it.**
     `tmp/t1410/wedged-agent.py` is the model: make it fire on a known-present
     phenomenon and stay silent on a known-absent one.
  3. **No surprising finding enters a doc on one instrument's word** when a
     second exists. Already fork doctrine — *take the screenshot before believing
     `warpctrl agent read`* — and it generalises.
- **commit before each risky phase**, so a silent death still leaves a coherent
  trail on `dev`.
- **two consecutive failed attempts at the same thing.** Write down what was
  tried and move to the next item. Thrashing overnight produces a long
  transcript and no findings.
- any finding that contradicts a security claim in this fork's own docs. Those
  claims are the reason the fork exists; one going false outranks whatever
  ticket was in hand.
- anything that would leave a Warp or agent process running at the end.

**And the rule that makes unattended work worth reading in the morning:** every
commit body says what was *found*, including what was found to be wrong. A night
of green tests with no findings is a night that produced nothing.

## Confirmed working — what makes this reachable

**Recovery is total.** `agent cancel` ends a wedge and the next turn's
`session/load` restores the conversation *including work done in the minutes
before it stalled*. Conversations survived Warp being closed and rebuilt twice.
**A wedge costs time, not state** — and since T14.10 it costs less time, because
the CLI now says one is happening.

**Answering is cheap.** `agent approvals` prints the exact command that answers
each request, with the yes omitted where there is none and the digest carried
through, so consent is a paste rather than two transcriptions.

## Ruled out, with the measurement

**Replay cost at length is not a problem.** `session/load` took 0.34s at one
exchange of history and 0.34s at six, with process startup dominating. Growth is
two notifications per exchange. Unverified: fifty exchanges, and large tool
outputs rather than short text.

**Relaxing the permission allowlist is not the answer to unanswerable requests.**
A person's yes on a shown request would be sound — that machinery exists and was
checked — but after the config remedy the residual is one agent, outside the
project, unconfigured, once. Left on the shelf with the counts that would justify
building it, in T14.8.

## Not this weekend

T10's upstream merge and I22's OpenRouter provider (that entry was numbered I18 when this was written; renumbered 2026-09-01 to end a collision). Merging while the agent
surface moves would confuse two kinds of breakage; the merge cost is paid by
deferral, not by divergence. A deliberate wait.

## Guardrails

- Commit each increment on `dev`, findings in the body. **No push, no PR, no
  upstream merge.**
- `CARGO_BUILD_JOBS=8` on every release build — uncapped takes the WSL VM down.
- Leave no Warp or agent processes running. Stop with `warpctrl window close`,
  and **cancel a wedged turn first or it will not close**.
- Scratch profiles via `XDG_CONFIG_HOME`/`XDG_STATE_HOME` only. **Never** touch
  `~/.claude/settings.json` or the user's `settings.toml`.
- **`cd` the pane into the repo before the first prompt.** A fresh pane starts in
  `$HOME`, both agent paths take the session cwd from the pane, and for an ACP
  agent that directory also decides whether its permission rules load at all.
- Verify by running. **Name the inputs that were not verified.**
