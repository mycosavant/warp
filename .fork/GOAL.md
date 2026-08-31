# The horizon: the fork builds the fork, and keeps going when nobody is watching

**Set 2026-08-29, replacing the horizon met the same day.** Delete this file when
it is met or abandoned — it is a horizon, not doctrine. **Read it first; it
outranks `TASKS.md`'s ordering while it stands.**

---

## Where this starts

The previous horizon asked for *one* multi-turn conversation in Warp's own panel
that changed the fork, asked permission, was answered, and remembered itself.
That was met, and then three tickets went at what it cost:

- **T14.7** proved it possible. **T14.9** ran seven turns of real work and
  produced the friction log everything since has aimed at.
- **T14.8** answered the worst of it. A permission request Warp could not answer
  turned out to be one agent's way of asking to leave the project directory —
  knowable in advance, fixable in the agent's own config, and now refused in
  words that say what Warp *cannot tell* rather than implying the call is
  dangerous. Answering the ones it can is now a single paste.
- **T14.10** gave a silent turn a voice: `quiet_for_seconds`, `last_activity`
  and `waiting_for_you`, plus the discovery that a wedged turn blocks `window
  close` until you cancel it. The third field exists because using the first two
  produced a false alarm within the hour — a turn waiting on *me* looked exactly
  like a wedge.

So the loop works and the sharp edges named by one real session are filed off.
**What has never happened is a second real session.**

## Destination

> **A working day on the fork, driven from the fork.** Not one conversation and
> not a demonstration: enough real work, over enough turns, that the friction log
> stops being a list of blockers and becomes a list of preferences.

Met when **T14.11** produces a commit made that way *and* its friction log
contains nothing that stops a turn. Frictions that merely annoy are the success
condition, not a failure — that is the whole difference between this horizon and
the last one.

## The order, and why

**T14.11 first, again.** Two rounds of fixes have landed on a measurement nobody
has re-taken. Running first has overturned the plan twice in T14 — Phase 0's
table was wrong in both cells, and T14.9 demoted the button that everyone was
sure about — so the prior is strong and it is cheap to honour.

~~The session's own task should be **T14.15**~~ — **done before the session, in
about an hour, because the audit trail has to exist before an unattended run can
rely on one.** An orchestrator answering an agent's prompts is only defensible if
every approval traces afterwards to the edit it produced, and that trail was
split across two files that did not name each other. One turn now writes one
file.

So the session's task is **T14.14** (model selection, which the protocol already
offers) — tractable, self-contained, and real. **T14.13 is measured by the same
run rather than built by it**: its first step is *"drive a session long enough to
hit the context limit and record what the panel does"*, which is the long session
itself. A dogfood needs something to be building while it gets long; T14.14 is
that, and the cliff arrives for free or does not arrive, and either is a finding.
**T14.12 is closed without being built** — polled through a live turn, `agent
read` streams after all (0 → 675 → 742 → 838 characters, `is_complete: false`
throughout), so T14.9's claim that it shows nothing until a turn ends was simply
wrong. That is the third time in T14 that measuring first deleted the work
instead of guiding it, and it is the argument for this ordering in one line.

**T14.16 landed out of order, on request: the panel can answer.** Two taps for
yes, one for no, the reason shown where there is no yes — verified by clicking
it. The argument that shelved it in T14.8 was overturned twice: the greyed-out
case is one agent's convention rather than a protocol fact, and T14.11 showed
there is no unattended answerer, which makes a present person answering cheaply
the only mechanism rather than a nicety. **That also changes what the horizon's
destination means** — "driven from the fork" has quietly meant "with a person
answering", and now there is somewhere for that person to click.

**A run said to, 2026-08-30. T14.13 moved ahead of T14.11, and T14.11 is now
person-present by construction.** Unattended, all three doors to a panel-made
commit are shut — the harness refusal, the flagship agent (whose `auto` mode
never asks Warp and whose `default` mode wedges with no answerer, so it is
unusable in *both* directions), and the permission freeze. That is the consent
design working, not a defect, and it makes T14.11 a task that waits for a person
at the T14.16 button rather than one to be rewritten as impossible.

The night went to T14.13 instead, and it inverted the ticket: **the agent
compacts itself every five or six turns and neither it nor Warp says a word.**
Warp's transcript stays complete while the agent's context does not, and the
panel shows the complete one. T14.9's seven-turn session — the measurement this
whole phase was built on — sat right at the first compaction boundary.

**So T14.11 should now run after T14.13's disclosure lands, not before it.** A
working day measured across three invisible compactions would produce a friction
log nobody could trust, which is the one outcome this horizon cannot afford.

**Reorder this list if a run says to.** That is what it is for.

## Working unattended

**This section is the charter, and it exists because the maintainer is away.**
Everything below is already true of this fork; it is written down here because
the usual backstop — someone reading the next message — is missing.

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

T10's upstream merge and I18's OpenRouter provider. Merging while the agent
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
